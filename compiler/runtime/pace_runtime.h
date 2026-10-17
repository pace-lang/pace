#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#include <stddef.h>
#include <stdint.h>
#include <pthread.h>

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

// Collections API
struct PaceList;
struct PaceMap;

typedef struct PaceList PaceList;
typedef struct PaceMap PaceMap;

PaceList* pace_list_new(size_t capacity, void (*elem_deinit)(void*));
void pace_list_push(PaceList* list, void* val);
void* pace_list_pop(PaceList* list);
void* pace_list_get(PaceList* list, int64_t index);
void pace_list_set(PaceList* list, int64_t index, void* val);
int64_t pace_list_len(PaceList* list);
void pace_list_clear(PaceList* list);

PaceMap* pace_map_new(size_t capacity, int key_is_string, void (*val_deinit)(void*));
void pace_map_set_int(PaceMap* map, int64_t key, void* val);
void* pace_map_get_int(PaceMap* map, int64_t key, int* found);
int pace_map_has_int(PaceMap* map, int64_t key);
int pace_map_remove_int(PaceMap* map, int64_t key);
void pace_map_set_str(PaceMap* map, struct PaceString* key, void* val);
void* pace_map_get_str(PaceMap* map, struct PaceString* key, int* found);
int pace_map_has_str(PaceMap* map, struct PaceString* key);
int pace_map_remove_str(PaceMap* map, struct PaceString* key);
int64_t pace_map_len(PaceMap* map);
void pace_map_clear(PaceMap* map);

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
