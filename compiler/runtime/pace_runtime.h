#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Pace Runtime FFI

void pace_panic(const char* msg);
void* pace_alloc(size_t size, void (*deinit)(void*));
void pace_retain(void* obj);
void pace_release(void* obj);

struct PaceString {
    char* data;
    size_t length;
};

struct PaceString* pace_string_new(const char* c_str);
struct PaceString* pace_format_string(const char* fmt, ...);

void pace_print_int(long long val);
void pace_print_float(double val);
void pace_print_bool(int val);
void pace_print_string(struct PaceString* val);
void pace_println();

// Concurrency API
// Event Loop (Single-threaded)
void pace_event_loop_init(void);
void pace_event_loop_run(void);
void* pace_spawn_fiber(void (*func)(void*), void* arg);
void* pace_await_fiber(void* fiber_ptr);
void pace_fiber_set_result(void* fiber_ptr, void* result);
void pace_fiber_set_current_result(void* result);
void* pace_sleep(long long ms);

// Thread Pool (for Actors)
void pace_thread_pool_init(size_t num_threads);
void pace_thread_pool_shutdown(void);

// Actor Mailbox implementation
struct PaceActorMessage {
    void (*func)(void*);
    void* arg;
    void* future; // PaceFiber*
    struct PaceActorMessage* next;
};

struct PaceActorBase {
    void* vtable;
    pthread_mutex_t mailbox_mutex;
    struct PaceActorMessage* mailbox_head;
    struct PaceActorMessage* mailbox_tail;
    int is_running;
};

void* pace_send_actor_message(void* actor, void (*func)(void*), void* arg);

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
