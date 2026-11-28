#define _XOPEN_SOURCE 600
#ifndef _WIN32
#include <stdlib.h>
#include <stdio.h>
#include <pthread.h>
#include <ucontext.h>
#include <time.h>
#include <unistd.h>
#include "../pace_runtime.h"

#define FIBER_STACK_SIZE (64 * 1024)

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

static void* pace_xmalloc(size_t size) {
    void* ptr = malloc(size);
    if (!ptr) {
        fprintf(stderr, "Fatal error: out of memory in concurrency runtime\n");
        abort();
    }
    return ptr;
}

void* pace_spawn_fiber(void (*func)(void*), void* arg) {
    PaceFiber* fiber = (PaceFiber*)pace_xmalloc(sizeof(PaceFiber));
    fiber->stack = pace_xmalloc(FIBER_STACK_SIZE);
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
    PaceFiber* fiber = (PaceFiber*)pace_xmalloc(sizeof(PaceFiber));
    fiber->stack = pace_xmalloc(FIBER_STACK_SIZE);
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

static size_t configured_thread_count = 4;

void pace_thread_pool_init(size_t num_threads) {
    configured_thread_count = (num_threads == 0) ? 4 : num_threads;
}

static void ensure_thread_pool_started(void) {
    if (worker_threads) return;
    size_t count = (configured_thread_count == 0) ? 4 : configured_thread_count;
    worker_count = count;
    worker_threads = (pthread_t*)pace_xmalloc(sizeof(pthread_t) * count);
    for (size_t i = 0; i < count; i++) {
        pthread_create(&worker_threads[i], NULL, worker_thread_main, NULL);
    }
}

void* pace_send_actor_message(void* actor_ptr, void (*func)(void*), void* arg) {
    struct PaceActorBase* actor = (struct PaceActorBase*)actor_ptr;
    
    PaceFiber* future = (PaceFiber*)pace_xmalloc(sizeof(PaceFiber));
    future->stack = NULL;
    future->is_done = 0;
    future->result = NULL;
    future->waiting_on = NULL;
    future->next = NULL;
    register_fiber(future);

    struct PaceActorMessage* msg = (struct PaceActorMessage*)pace_xmalloc(sizeof(struct PaceActorMessage));
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
        ThreadTask* task = (ThreadTask*)pace_xmalloc(sizeof(ThreadTask));
        task->actor = actor;
        task->next = NULL;
        
        pthread_mutex_lock(&queue_mutex);
        ensure_thread_pool_started();
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
    if (worker_threads) {
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
    }
    
    cleanup_all_fibers();
}


#else // _WIN32

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "../pace_runtime.h"

#define FIBER_STACK_SIZE (64 * 1024)

typedef struct PaceFiber {
    LPVOID win_fiber;
    void (*func)(void*);
    void* arg;
    long long sleep_ms;
    int is_sleep;
    volatile int is_done;
    void* result;
    struct PaceFiber* waiting_on;
    struct PaceFiber* next;
    int awaited;
    struct PaceFiber* all_next;
} PaceFiber;

typedef struct SleepEntry {
    PaceFiber* fiber;
    long long wakeup_time_ms;
    struct SleepEntry* next;
} SleepEntry;

static PaceFiber* all_fibers_head = NULL;
static CRITICAL_SECTION all_fibers_cs;
static int all_fibers_cs_init = 0;

#ifdef _MSC_VER
static __declspec(thread) PaceFiber* current_fiber = NULL;
#else
static __thread PaceFiber* current_fiber = NULL;
#endif

static PaceFiber* ready_queue_head = NULL;
static PaceFiber* ready_queue_tail = NULL;
static SleepEntry* sleep_queue = NULL;
static LPVOID main_loop_fiber = NULL;

static void ensure_fibers_cs(void) {
    if (!all_fibers_cs_init) {
        InitializeCriticalSection(&all_fibers_cs);
        all_fibers_cs_init = 1;
    }
}

static void ensure_main_fiber(void) {
    if (!main_loop_fiber) {
        main_loop_fiber = ConvertThreadToFiber(NULL);
        if (!main_loop_fiber) {
            main_loop_fiber = GetCurrentFiber();
        }
    }
}

static void* pace_xmalloc(size_t size) {
    void* ptr = malloc(size);
    if (!ptr) {
        fprintf(stderr, "Fatal error: out of memory in concurrency runtime\n");
        abort();
    }
    return ptr;
}

static void register_fiber(PaceFiber* fiber) {
    ensure_fibers_cs();
    fiber->awaited = 0;
    fiber->all_next = NULL;
    EnterCriticalSection(&all_fibers_cs);
    fiber->all_next = all_fibers_head;
    all_fibers_head = fiber;
    LeaveCriticalSection(&all_fibers_cs);
}

static void cleanup_all_fibers(void) {
    if (!all_fibers_cs_init) return;
    EnterCriticalSection(&all_fibers_cs);
    PaceFiber* curr = all_fibers_head;
    while (curr) {
        PaceFiber* next = curr->all_next;
        if (curr->win_fiber) {
            DeleteFiber(curr->win_fiber);
            curr->win_fiber = NULL;
        }
        free(curr);
        curr = next;
    }
    all_fibers_head = NULL;
    LeaveCriticalSection(&all_fibers_cs);
}

static long long current_time_ms(void) {
    return (long long)GetTickCount64();
}

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

static VOID CALLBACK win_fiber_trampoline(PVOID lpParameter) {
    PaceFiber* fiber = (PaceFiber*)lpParameter;
    if (fiber->is_sleep) {
        SleepEntry* entry = (SleepEntry*)pace_xmalloc(sizeof(SleepEntry));
        entry->fiber = fiber;
        entry->wakeup_time_ms = current_time_ms() + fiber->sleep_ms;
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
        fiber->waiting_on = (void*)1;
        SwitchToFiber(main_loop_fiber);
        fiber->waiting_on = NULL;
    } else if (fiber->func) {
        fiber->func(fiber->arg);
    }
    fiber->is_done = 1;
    SwitchToFiber(main_loop_fiber);
}

void pace_event_loop_init(void) {
    ready_queue_head = NULL;
    ready_queue_tail = NULL;
    sleep_queue = NULL;
    current_fiber = NULL;
    ensure_fibers_cs();
    ensure_main_fiber();
}

void* pace_spawn_fiber(void (*func)(void*), void* arg) {
    ensure_main_fiber();
    PaceFiber* fiber = (PaceFiber*)pace_xmalloc(sizeof(PaceFiber));
    fiber->func = func;
    fiber->arg = arg;
    fiber->sleep_ms = 0;
    fiber->is_sleep = 0;
    fiber->is_done = 0;
    fiber->result = NULL;
    fiber->waiting_on = NULL;
    fiber->next = NULL;
    fiber->win_fiber = CreateFiber(FIBER_STACK_SIZE, win_fiber_trampoline, fiber);
    if (!fiber->win_fiber) {
        fprintf(stderr, "Fatal error: CreateFiber failed in pace_spawn_fiber\n");
        abort();
    }
    register_fiber(fiber);
    enqueue_fiber(fiber);
    return fiber;
}

void* pace_await_fiber(void* fiber_ptr) {
    if (!fiber_ptr) return NULL;
    ensure_main_fiber();
    PaceFiber* target = (PaceFiber*)fiber_ptr;

    if (target->awaited) {
        return target->result;
    }

    if (!target->is_done) {
        if (current_fiber) {
            current_fiber->waiting_on = target;
            SwitchToFiber(main_loop_fiber);
        } else {
            while (!target->is_done) {
                pace_check_sleep_queue();
                PaceFiber* next_f = dequeue_fiber();
                if (next_f) {
                    current_fiber = next_f;
                    SwitchToFiber(next_f->win_fiber);
                    current_fiber = NULL;
                    if (!next_f->is_done && next_f->waiting_on != (void*)1) {
                        enqueue_fiber(next_f);
                    }
                } else if (sleep_queue) {
                    long long now = current_time_ms();
                    long long sleep_time = sleep_queue->wakeup_time_ms - now;
                    if (sleep_time > 0) {
                        Sleep((DWORD)(sleep_time > 10 ? 10 : sleep_time));
                    }
                } else {
                    Sleep(1);
                }
            }
        }
    }

    void* result = target->result;
    target->awaited = 1;
    if (target->win_fiber) {
        DeleteFiber(target->win_fiber);
        target->win_fiber = NULL;
    }
    return result;
}

void pace_event_loop_run(void) {
    ensure_main_fiber();
    while (ready_queue_head || sleep_queue) {
        pace_check_sleep_queue();
        PaceFiber* f = dequeue_fiber();
        if (f) {
            if (f->waiting_on && f->waiting_on != (void*)1 && !f->waiting_on->is_done) {
                enqueue_fiber(f);
                if (ready_queue_head == f) {
                    Sleep(1);
                }
                continue;
            } else {
                f->waiting_on = NULL;
            }

            current_fiber = f;
            SwitchToFiber(f->win_fiber);
            current_fiber = NULL;

            if (!f->is_done && f->waiting_on != (void*)1) {
                enqueue_fiber(f);
            }
        } else if (sleep_queue) {
            long long now = current_time_ms();
            long long sleep_time = sleep_queue->wakeup_time_ms - now;
            if (sleep_time > 0) {
                Sleep((DWORD)sleep_time);
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

void* pace_sleep(long long ms) {
    ensure_main_fiber();
    PaceFiber* fiber = (PaceFiber*)pace_xmalloc(sizeof(PaceFiber));
    fiber->func = NULL;
    fiber->arg = NULL;
    fiber->sleep_ms = ms;
    fiber->is_sleep = 1;
    fiber->is_done = 0;
    fiber->result = NULL;
    fiber->waiting_on = NULL;
    fiber->next = NULL;
    fiber->win_fiber = CreateFiber(FIBER_STACK_SIZE, win_fiber_trampoline, fiber);
    if (!fiber->win_fiber) {
        fprintf(stderr, "Fatal error: CreateFiber failed in pace_sleep\n");
        abort();
    }
    register_fiber(fiber);
    enqueue_fiber(fiber);
    return fiber;
}

void pace_thread_pool_init(size_t num_threads) {
    (void)num_threads;
}

void* pace_send_actor_message(void* actor_ptr, void (*func)(void*, void*), void* arg) {
    (void)actor_ptr;
    (void)func;
    (void)arg;
    fprintf(stderr, "Actors (Multi-Threading) are not currently supported on Windows.\n");
    exit(1);
}

void pace_thread_pool_shutdown(void) {
    cleanup_all_fibers();
}

#endif // _WIN32
