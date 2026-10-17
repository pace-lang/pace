#define _XOPEN_SOURCE 600
#ifndef _WIN32
#include <stdlib.h>
#include <stdio.h>
#include <pthread.h>
#include <ucontext.h>
#include <time.h>
#include <unistd.h>
#include "../pace_runtime.h"

#define FIBER_STACK_SIZE (1024 * 1024)

typedef struct PaceFiber {
    ucontext_t ctx;
    void* stack;
    volatile int is_done;
    void* result;
    struct PaceFiber* waiting_on;
    struct PaceFiber* next;
    int awaited;
    struct PaceFiber* all_next;
} PaceFiber;

static PaceFiber* all_fibers_head = NULL;
static pthread_mutex_t all_fibers_mutex = PTHREAD_MUTEX_INITIALIZER;

static void register_fiber(PaceFiber* fiber) {
    fiber->awaited = 0;
    fiber->all_next = NULL;
    pthread_mutex_lock(&all_fibers_mutex);
    fiber->all_next = all_fibers_head;
    all_fibers_head = fiber;
    pthread_mutex_unlock(&all_fibers_mutex);
}

static void cleanup_all_fibers(void) {
    pthread_mutex_lock(&all_fibers_mutex);
    PaceFiber* curr = all_fibers_head;
    while (curr) {
        PaceFiber* next = curr->all_next;
        if (curr->stack) {
            free(curr->stack);
            curr->stack = NULL;
        }
        free(curr);
        curr = next;
    }
    all_fibers_head = NULL;
    pthread_mutex_unlock(&all_fibers_mutex);
}

typedef struct SleepEntry {
    PaceFiber* fiber;
    long long wakeup_time_ms;
    struct SleepEntry* next;
} SleepEntry;

static __thread PaceFiber* current_fiber = NULL;
static PaceFiber* ready_queue_head = NULL;
static PaceFiber* ready_queue_tail = NULL;
static SleepEntry* sleep_queue = NULL;
static ucontext_t main_loop_ctx;

static long long current_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

// Queue management
static void enqueue_fiber(PaceFiber* fiber) {
    fiber->next = NULL;
    if (ready_queue_tail) {
        ready_queue_tail->next = fiber;
        ready_queue_tail = fiber;
    } else {
        ready_queue_head = ready_queue_tail = fiber;
    }
}

static PaceFiber* dequeue_fiber(void) {
    if (!ready_queue_head) return NULL;
    PaceFiber* f = ready_queue_head;
    ready_queue_head = f->next;
    if (!ready_queue_head) ready_queue_tail = NULL;
    return f;
}

void pace_event_loop_init(void) {
    ready_queue_head = NULL;
    ready_queue_tail = NULL;
    sleep_queue = NULL;
    current_fiber = NULL;
}

// Trampoline to run fiber func
static void fiber_trampoline(void (*func)(void*), void* arg, PaceFiber* fiber) {
    func(arg);
    fiber->is_done = 1;
    // Switch back to the main loop context when done
    setcontext(&main_loop_ctx);
}

void* pace_spawn_fiber(void (*func)(void*), void* arg) {
    PaceFiber* fiber = (PaceFiber*)malloc(sizeof(PaceFiber));
    fiber->stack = malloc(FIBER_STACK_SIZE);
    fiber->is_done = 0;
    fiber->result = NULL;
    fiber->waiting_on = NULL;
    fiber->next = NULL;
    register_fiber(fiber);

    getcontext(&fiber->ctx);
    fiber->ctx.uc_stack.ss_sp = fiber->stack;
    fiber->ctx.uc_stack.ss_size = FIBER_STACK_SIZE;
    fiber->ctx.uc_link = &main_loop_ctx;
    
    makecontext(&fiber->ctx, (void (*)())fiber_trampoline, 3, func, arg, fiber);

    enqueue_fiber(fiber);
    return fiber;
}

static void pace_check_sleep_queue(void) {
    long long now = current_time_ms();
    SleepEntry* prev = NULL;
    SleepEntry* curr = sleep_queue;
    while (curr) {
        if (now >= curr->wakeup_time_ms) {
            enqueue_fiber(curr->fiber);
            SleepEntry* to_free = curr;
            if (prev) {
                prev->next = curr->next;
                curr = curr->next;
            } else {
                sleep_queue = curr->next;
                curr = curr->next;
            }
            free(to_free);
        } else {
            prev = curr;
            curr = curr->next;
        }
    }
}

void* pace_await_fiber(void* fiber_ptr) {
    if (!fiber_ptr) return NULL;
    PaceFiber* target = (PaceFiber*)fiber_ptr;
    
    if (target->awaited) {
        return target->result;
    }

    if (!target->is_done) {
        if (current_fiber) {
            current_fiber->waiting_on = target;
            // Save current fiber state and switch back to main loop
            swapcontext(&current_fiber->ctx, &main_loop_ctx);
        } else {
            // If called from main thread
            while (!target->is_done) {
                pace_check_sleep_queue();
                PaceFiber* next_f = dequeue_fiber();
                if (next_f) {
                    current_fiber = next_f;
                    swapcontext(&main_loop_ctx, &next_f->ctx);
                    current_fiber = NULL;
                    if (!next_f->is_done && next_f->waiting_on != (void*)1) {
                        enqueue_fiber(next_f);
                    }
                } else if (sleep_queue) {
                    long long now = current_time_ms();
                    long long sleep_time = sleep_queue->wakeup_time_ms - now;
                    if (sleep_time > 0) {
                        usleep(sleep_time > 10 ? 10 * 1000 : sleep_time * 1000);
                    }
                } else {
                    usleep(500);
                }
            }
        }
    }
    
    void* result = target->result;
    target->awaited = 1;
    if (target->stack) {
        free(target->stack);
        target->stack = NULL;
    }
    return result;
}

void pace_event_loop_run(void) {
    while (ready_queue_head || sleep_queue) {
        pace_check_sleep_queue();
        
        PaceFiber* f = dequeue_fiber();
        if (f) {
            if (f->waiting_on && f->waiting_on != (void*)1 && !f->waiting_on->is_done) {
                enqueue_fiber(f);
                if (ready_queue_head == f) {
                    usleep(500);
                }
                continue;
            } else {
                f->waiting_on = NULL;
            }

            current_fiber = f;
            swapcontext(&main_loop_ctx, &f->ctx);
            current_fiber = NULL;

            if (!f->is_done && f->waiting_on != (void*)1) {
                enqueue_fiber(f);
            }
        } else if (sleep_queue) {
            long long now = current_time_ms();
            long long sleep_time = sleep_queue->wakeup_time_ms - now;
            if (sleep_time > 0) {
                usleep(sleep_time * 1000);
            }
        }
    }
}

void pace_fiber_set_result(void* fiber_ptr, void* result) {
    PaceFiber* target = (PaceFiber*)fiber_ptr;
    if (target) {
        target->result = result;
    }
}

void pace_fiber_set_current_result(void* result) {
    if (current_fiber) {
        current_fiber->result = result;
    }
}

static void fiber_sleep_trampoline(void* arg) {
    long long ms = (long long)arg;
    
    SleepEntry* entry = (SleepEntry*)malloc(sizeof(SleepEntry));
    entry->fiber = current_fiber;
    entry->wakeup_time_ms = current_time_ms() + ms;
    
    // insert sorted by wakeup time
    if (!sleep_queue || sleep_queue->wakeup_time_ms >= entry->wakeup_time_ms) {
        entry->next = sleep_queue;
        sleep_queue = entry;
    } else {
        SleepEntry* curr = sleep_queue;
        while (curr->next && curr->next->wakeup_time_ms < entry->wakeup_time_ms) {
            curr = curr->next;
        }
        entry->next = curr->next;
        curr->next = entry;
    }
    
    // Prevent event loop from immediately re-enqueueing this fiber
    current_fiber->waiting_on = (void*)1; 
    
    swapcontext(&current_fiber->ctx, &main_loop_ctx);
    
    // Woken up
    current_fiber->waiting_on = NULL;
    current_fiber->is_done = 1;
    setcontext(&main_loop_ctx);
}

void* pace_sleep(long long ms) {
    PaceFiber* fiber = (PaceFiber*)malloc(sizeof(PaceFiber));
    fiber->stack = malloc(FIBER_STACK_SIZE);
    fiber->is_done = 0;
    fiber->result = NULL;
    fiber->waiting_on = NULL;
    fiber->next = NULL;
    register_fiber(fiber);

    getcontext(&fiber->ctx);
    fiber->ctx.uc_stack.ss_sp = fiber->stack;
    fiber->ctx.uc_stack.ss_size = FIBER_STACK_SIZE;
    fiber->ctx.uc_link = &main_loop_ctx;
    
    makecontext(&fiber->ctx, (void (*)())fiber_sleep_trampoline, 1, (void*)ms);

    enqueue_fiber(fiber);
    return fiber;
}

// Thread Pool Implementation
typedef struct ThreadTask {
    struct PaceActorBase* actor;
    struct ThreadTask* next;
} ThreadTask;

static pthread_t* worker_threads = NULL;
static size_t worker_count = 0;
static ThreadTask* task_queue_head = NULL;
static ThreadTask* task_queue_tail = NULL;
static pthread_mutex_t queue_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t queue_cond = PTHREAD_COND_INITIALIZER;
static int pool_shutdown = 0;

static void* worker_thread_main(void* arg) {
    (void)arg;
    while (1) {
        pthread_mutex_lock(&queue_mutex);
        while (!task_queue_head && !pool_shutdown) {
            pthread_cond_wait(&queue_cond, &queue_mutex);
        }
        
        if (pool_shutdown && !task_queue_head) {
            pthread_mutex_unlock(&queue_mutex);
            break;
        }
        
        ThreadTask* task = task_queue_head;
        task_queue_head = task->next;
        if (!task_queue_head) task_queue_tail = NULL;
        pthread_mutex_unlock(&queue_mutex);
        
        if (task) {
            struct PaceActorBase* actor = task->actor;
            free(task);
            
            while (1) {
                pthread_mutex_lock(&actor->mailbox_mutex);
                struct PaceActorMessage* msg = actor->mailbox_head;
                if (msg) {
                    actor->mailbox_head = msg->next;
                    if (!actor->mailbox_head) {
                        actor->mailbox_tail = NULL;
                    }
                } else {
                    actor->is_running = 0;
                    pthread_mutex_unlock(&actor->mailbox_mutex);
                    break;
                }
                pthread_mutex_unlock(&actor->mailbox_mutex);
                
                current_fiber = (PaceFiber*)msg->future;
                msg->func(msg->arg);
                
                if (msg->future) {
                    ((PaceFiber*)msg->future)->is_done = 1;
                }
                current_fiber = NULL;
                free(msg);
            }
        }
    }
    return NULL;
}

void pace_thread_pool_init(size_t num_threads) {
    if (num_threads == 0) num_threads = 4; // Default to 4 threads
    worker_count = num_threads;
    worker_threads = (pthread_t*)malloc(sizeof(pthread_t) * num_threads);
    for (size_t i = 0; i < num_threads; i++) {
        pthread_create(&worker_threads[i], NULL, worker_thread_main, NULL);
    }
}

void* pace_send_actor_message(void* actor_ptr, void (*func)(void*), void* arg) {
    struct PaceActorBase* actor = (struct PaceActorBase*)actor_ptr;
    
    PaceFiber* future = (PaceFiber*)malloc(sizeof(PaceFiber));
    future->stack = NULL;
    future->is_done = 0;
    future->result = NULL;
    future->waiting_on = NULL;
    future->next = NULL;
    register_fiber(future);

    struct PaceActorMessage* msg = (struct PaceActorMessage*)malloc(sizeof(struct PaceActorMessage));
    msg->func = func;
    msg->arg = arg;
    msg->future = future;
    msg->next = NULL;
    
    pthread_mutex_lock(&actor->mailbox_mutex);
    if (actor->mailbox_tail) {
        actor->mailbox_tail->next = msg;
        actor->mailbox_tail = msg;
    } else {
        actor->mailbox_head = actor->mailbox_tail = msg;
    }
    
    int should_spawn = 0;
    if (!actor->is_running) {
        actor->is_running = 1;
        should_spawn = 1;
    }
    pthread_mutex_unlock(&actor->mailbox_mutex);
    
    if (should_spawn) {
        ThreadTask* task = (ThreadTask*)malloc(sizeof(ThreadTask));
        task->actor = actor;
        task->next = NULL;
        
        pthread_mutex_lock(&queue_mutex);
        if (task_queue_tail) {
            task_queue_tail->next = task;
            task_queue_tail = task;
        } else {
            task_queue_head = task_queue_tail = task;
        }
        pthread_cond_signal(&queue_cond);
        pthread_mutex_unlock(&queue_mutex);
    }
    
    return future;
}

void pace_thread_pool_shutdown(void) {
    if (!worker_threads) return;
    
    pthread_mutex_lock(&queue_mutex);
    pool_shutdown = 1;
    pthread_cond_broadcast(&queue_cond);
    pthread_mutex_unlock(&queue_mutex);
    
    for (size_t i = 0; i < worker_count; i++) {
        pthread_join(worker_threads[i], NULL);
    }
    
    free(worker_threads);
    worker_threads = NULL;
    worker_count = 0;
    
    cleanup_all_fibers();
}


#else // _WIN32

#include <stdio.h>
#include <stdlib.h>

void pace_event_loop_init(void) {
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void* pace_spawn_fiber(void (*func)(void*, void*), void* arg) {
    (void)func;
    (void)arg;
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void* pace_await_fiber(void* fiber_ptr) {
    (void)fiber_ptr;
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void pace_event_loop_run(void) {
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void pace_fiber_set_result(void* fiber_ptr, void* result) {
    (void)fiber_ptr;
    (void)result;
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void pace_fiber_set_current_result(void* result) {
    (void)result;
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void* pace_sleep(long long ms) {
    (void)ms;
    fprintf(stderr, "Concurrency (Fibers/Threads) is not currently supported on Windows.\n");
    exit(1);
}

void pace_thread_pool_init(size_t num_threads) {
    (void)num_threads;
    // On Windows, silently succeed init but abort if they actually spawn an actor message
    // (This allows single-threaded scripts to compile/run without crashing immediately if init is called by default)
}

void* pace_send_actor_message(void* actor_ptr, void (*func)(void*, void*), void* arg) {
    (void)actor_ptr;
    (void)func;
    (void)arg;
    fprintf(stderr, "Actors (Multi-Threading) are not currently supported on Windows.\n");
    exit(1);
}

void pace_thread_pool_shutdown(void) {
}

#endif // _WIN32
