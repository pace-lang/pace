#include <stdlib.h>
#include <stdio.h>
#include <pthread.h>
#include <ucontext.h>

#define FIBER_STACK_SIZE (1024 * 1024)

typedef struct PaceFiber {
    ucontext_t ctx;
    void* stack;
    int is_done;
    void* result;
    struct PaceFiber* waiting_on;
    struct PaceFiber* next;
} PaceFiber;

static PaceFiber* current_fiber = NULL;
static PaceFiber* ready_queue_head = NULL;
static PaceFiber* ready_queue_tail = NULL;
static ucontext_t main_loop_ctx;

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

static PaceFiber* dequeue_fiber() {
    if (!ready_queue_head) return NULL;
    PaceFiber* f = ready_queue_head;
    ready_queue_head = f->next;
    if (!ready_queue_head) ready_queue_tail = NULL;
    return f;
}

void pace_event_loop_init(void) {
    ready_queue_head = NULL;
    ready_queue_tail = NULL;
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

    getcontext(&fiber->ctx);
    fiber->ctx.uc_stack.ss_sp = fiber->stack;
    fiber->ctx.uc_stack.ss_size = FIBER_STACK_SIZE;
    fiber->ctx.uc_link = &main_loop_ctx;
    
    makecontext(&fiber->ctx, (void (*)())fiber_trampoline, 3, func, arg, fiber);

    enqueue_fiber(fiber);
    return fiber;
}

void* pace_await_fiber(void* fiber_ptr) {
    PaceFiber* target = (PaceFiber*)fiber_ptr;
    
    if (!target->is_done) {
        if (current_fiber) {
            current_fiber->waiting_on = target;
            // Save current fiber state and switch back to main loop
            swapcontext(&current_fiber->ctx, &main_loop_ctx);
        } else {
            // If called from main thread (not inside a fiber), we must run the event loop until it's done
            while (!target->is_done) {
                PaceFiber* next_f = dequeue_fiber();
                if (next_f) {
                    current_fiber = next_f;
                    swapcontext(&main_loop_ctx, &next_f->ctx);
                    current_fiber = NULL;
                    if (!next_f->is_done) {
                        enqueue_fiber(next_f);
                    }
                }
            }
        }
    }
    
    // Once done, retrieve result and clean up target fiber?
    // In a real GC'd language we wouldn't free immediately unless we track refs.
    // For now we assume one awaiter and free it.
    void* result = target->result;
    free(target->stack);
    free(target);
    return result;
}

void pace_event_loop_run(void) {
    while (ready_queue_head) {
        PaceFiber* f = dequeue_fiber();
        
        // If it's waiting on something that is not done, put it back
        if (f->waiting_on && !f->waiting_on->is_done) {
            enqueue_fiber(f);
            continue;
        } else {
            f->waiting_on = NULL;
        }

        current_fiber = f;
        swapcontext(&main_loop_ctx, &f->ctx);
        current_fiber = NULL;

        if (!f->is_done) {
            enqueue_fiber(f);
        }
    }
}

// API for fiber to set result before finishing
void pace_fiber_set_result(void* result) {
    if (current_fiber) {
        current_fiber->result = result;
    }
}
