#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#include <stddef.h>
#include <stdint.h>

#ifndef _WIN32
#include <pthread.h>
#else
typedef void* pthread_mutex_t;
#define PTHREAD_MUTEX_INITIALIZER NULL
#endif

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

typedef struct PaceResult {
    int is_ok;
    void* value;
    void* error;
} PaceResult;

struct PaceString* pace_string_new(const char* c_str);
struct PaceString* pace_format_string(const char* fmt, ...);

void pace_print_int(long long val);
void pace_print_float(double val);
void pace_print_bool(int val);
void pace_print_string(struct PaceString* val);
void pace_println();
void pace_assert(int cond, struct PaceString* msg);
int pace_string_equals(struct PaceString* a, struct PaceString* b);

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
struct PaceSet;

typedef struct PaceList PaceList;
typedef struct PaceMap PaceMap;
typedef struct PaceSet PaceSet;

PaceList* pace_list_new(size_t capacity, void (*elem_deinit)(void*));
void pace_list_push(PaceList* list, void* val);
void* pace_list_pop(PaceList* list);
void* pace_list_get(PaceList* list, int64_t index);
void pace_list_set(PaceList* list, int64_t index, void* val);
int64_t pace_list_len(PaceList* list);
void pace_list_clear(PaceList* list);
void pace_list_insert(PaceList* list, int64_t index, void* val);
void* pace_list_remove_at(PaceList* list, int64_t index);
int pace_list_contains_int(PaceList* list, int64_t val);
int pace_list_contains_str(PaceList* list, struct PaceString* val);
int pace_list_contains_ptr(PaceList* list, void* val);
int64_t pace_list_index_of_int(PaceList* list, int64_t val);
int64_t pace_list_index_of_str(PaceList* list, struct PaceString* val);
int64_t pace_list_index_of_ptr(PaceList* list, void* val);
PaceList* pace_list_slice(PaceList* list, int64_t start, int64_t end);
PaceList* pace_list_reversed(PaceList* list);
struct PaceString* pace_list_join(PaceList* list, struct PaceString* sep);

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
PaceList* pace_map_keys(PaceMap* map);
PaceList* pace_map_values(PaceMap* map);

PaceSet* pace_set_new(size_t capacity, int key_is_string);
int pace_set_add_int(PaceSet* set, int64_t key);
int pace_set_has_int(PaceSet* set, int64_t key);
int pace_set_remove_int(PaceSet* set, int64_t key);
int pace_set_add_str(PaceSet* set, struct PaceString* key);
int pace_set_has_str(PaceSet* set, struct PaceString* key);
int pace_set_remove_str(PaceSet* set, struct PaceString* key);
int pace_set_add_float(PaceSet* set, double key);
int pace_set_has_float(PaceSet* set, double key);
int pace_set_remove_float(PaceSet* set, double key);
int64_t pace_set_len(PaceSet* set);
void pace_set_clear(PaceSet* set);
PaceList* pace_set_to_list(PaceSet* set);
PaceSet* pace_set_union(PaceSet* a, PaceSet* b);
PaceSet* pace_set_intersection(PaceSet* a, PaceSet* b);
PaceSet* pace_set_difference(PaceSet* a, PaceSet* b);

// String & StringBuilder API
int64_t pace_string_len(struct PaceString* s);
int64_t pace_string_index_of(struct PaceString* s, struct PaceString* sub);
int pace_string_contains(struct PaceString* s, struct PaceString* sub);
int pace_string_starts_with(struct PaceString* s, struct PaceString* prefix);
int pace_string_ends_with(struct PaceString* s, struct PaceString* suffix);
struct PaceString* pace_string_trim(struct PaceString* s);
struct PaceString* pace_string_slice(struct PaceString* s, int64_t start, int64_t end);
struct PaceString* pace_string_to_lower(struct PaceString* s);
struct PaceString* pace_string_to_upper(struct PaceString* s);
struct PaceString* pace_string_replace(struct PaceString* s, struct PaceString* from, struct PaceString* to);
PaceList* pace_string_split(struct PaceString* s, struct PaceString* delim);

struct PaceStringBuilder;
typedef struct PaceStringBuilder PaceStringBuilder;
PaceStringBuilder* pace_sb_new(void);
void pace_sb_append(PaceStringBuilder* sb, struct PaceString* s);
struct PaceString* pace_sb_to_string(PaceStringBuilder* sb);
int64_t pace_sb_len(PaceStringBuilder* sb);
void pace_sb_clear(PaceStringBuilder* sb);

// Stdlib Runtime API
void pace_set_args(int argc, char** argv);
PaceList* pace_process_args(void);
struct PaceString* pace_process_cwd(void);
void pace_process_exit(int64_t code);
void pace_eprintln(struct PaceString* val);
PaceResult pace_read_line(void);
int pace_file_exists(struct PaceString* path);
PaceResult pace_file_read_text(struct PaceString* path);
PaceResult pace_file_write_text(struct PaceString* path, struct PaceString* contents);
int pace_dir_exists(struct PaceString* path);
PaceResult pace_dir_make(struct PaceString* path);
struct PaceString* pace_env_get(struct PaceString* key);
void pace_env_set(struct PaceString* key, struct PaceString* val);
int pace_env_has(struct PaceString* key);
struct PaceString* pace_os_name(void);
int64_t pace_time_now_ms(void);
void pace_time_sleep_ms(int64_t ms);
double pace_math_sqrt(double x);
double pace_math_pow(double x, double y);
double pace_math_abs(double x);
double pace_math_round(double x);
double pace_math_floor(double x);
double pace_math_ceil(double x);
double pace_math_sin(double x);
double pace_math_cos(double x);
double pace_math_tan(double x);

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
