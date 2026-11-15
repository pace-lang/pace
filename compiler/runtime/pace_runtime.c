#include "pace_runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <pthread.h>


void pace_println() {
    printf("\n");
}

void pace_assert(int cond, struct PaceString* msg) {
    if (!cond) {
        if (msg && msg->data) {
            fprintf(stderr, "Assertion failed: %.*s\n", (int)msg->length, msg->data);
        } else {
            fprintf(stderr, "Assertion failed\n");
        }
        abort();
    }
}

char* pace_format_string(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int size = vsnprintf(NULL, 0, fmt, args);
    va_end(args);
    char* buf = (char*)malloc(size + 1);
    va_start(args, fmt);
    vsnprintf(buf, size + 1, fmt, args);
    va_end(args);
    return buf;
}

void* pace_alloc(size_t size, pace_destructor_t deinit) {
    // Allocate space for the header + the object payload
    pace_arc_header* header = (pace_arc_header*)malloc(sizeof(pace_arc_header) + size);
    if (!header) {
        fprintf(stderr, "Fatal error: out of memory\n");
        exit(1);
    }
    
    header->ref_count = 1; // Objects start with a reference count of 1
    header->deinit = deinit;
    
    // Return a pointer to the payload, which sits immediately after the header
    return (void*)((char*)header + sizeof(pace_arc_header));
}

void pace_retain(void* obj) {
    if (!obj) return;
    pace_arc_header* hdr = ((pace_arc_header*)obj) - 1;
    hdr->ref_count++;
}

void pace_release(void* obj) {
    if (!obj) return;
    
    // Get the header, which is immediately before the object payload
    pace_arc_header* header = (pace_arc_header*)((char*)obj - sizeof(pace_arc_header));
    
    if (header->ref_count > 0) {
        header->ref_count--;
        
        if (header->ref_count == 0) {
            // Invoke the destructor if present
            if (header->deinit) {
                header->deinit(obj);
            }
            free(header);
        }
    }
}

void pace_print_int(long long val) {
    printf("%lld\n", val);
}

void pace_print_float(double val) {
    printf("%f\n", val);
}

void pace_print_bool(int val) {
    if (val) {
        printf("true\n");
    } else {
        printf("false\n");
    }
}

void pace_print_string(const char* val) {
    printf("%s\n", val);
}

int pace_string_equals(struct PaceString* a, struct PaceString* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (a->length != b->length) return 0;
    if (a->length == 0) return 1;
    if (!a->data || !b->data) return a->data == b->data;
    return memcmp(a->data, b->data, a->length) == 0;
}

// ---------------------------------------------------------
// Concurrency API
// ---------------------------------------------------------

typedef struct PaceTask {
    void (*func)(void*);
    void* arg;
    struct PaceTask* next;
} PaceTask;

// Event Loop State (Single-threaded)
static PaceTask* el_head = NULL;
static PaceTask* el_tail = NULL;

void pace_event_loop_init(void) {
    el_head = NULL;
    el_tail = NULL;
}

void pace_spawn_task(void (*func)(void*), void* arg) {
    PaceTask* task = (PaceTask*)malloc(sizeof(PaceTask));
    task->func = func;
    task->arg = arg;
    task->next = NULL;
    
    if (el_tail) {
        el_tail->next = task;
        el_tail = task;
    } else {
        el_head = el_tail = task;
    }
}

void pace_event_loop_run(void) {
    while (el_head) {
        PaceTask* task = el_head;
        el_head = el_head->next;
        if (!el_head) el_tail = NULL;
        
        task->func(task->arg);
        free(task);
    }
}

// Thread Pool State (for Actors)
static pthread_t* workers = NULL;
static size_t num_workers = 0;
static PaceTask* tp_head = NULL;
static PaceTask* tp_tail = NULL;
static pthread_mutex_t tp_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t tp_cond = PTHREAD_COND_INITIALIZER;
static int tp_shutdown = 0;

static void* worker_thread(void* arg) {
    while (1) {
        pthread_mutex_lock(&tp_mutex);
        while (!tp_head && !tp_shutdown) {
            pthread_cond_wait(&tp_cond, &tp_mutex);
        }
        
        if (tp_shutdown && !tp_head) {
            pthread_mutex_unlock(&tp_mutex);
            break;
        }
        
        PaceTask* task = tp_head;
        if (task) {
            tp_head = tp_head->next;
            if (!tp_head) tp_tail = NULL;
        }
        pthread_mutex_unlock(&tp_mutex);
        
        if (task) {
            task->func(task->arg);
            free(task);
        }
    }
    return NULL;
}

void pace_thread_pool_init(size_t num_threads) {
    num_workers = num_threads;
    workers = (pthread_t*)malloc(sizeof(pthread_t) * num_threads);
    for (size_t i = 0; i < num_threads; i++) {
        pthread_create(&workers[i], NULL, worker_thread, NULL);
    }
}

void pace_spawn_actor_task(void (*func)(void*), void* arg) {
    PaceTask* task = (PaceTask*)malloc(sizeof(PaceTask));
    task->func = func;
    task->arg = arg;
    task->next = NULL;
    
    pthread_mutex_lock(&tp_mutex);
    if (tp_tail) {
        tp_tail->next = task;
        tp_tail = task;
    } else {
        tp_head = tp_tail = task;
    }
    pthread_cond_signal(&tp_cond);
    pthread_mutex_unlock(&tp_mutex);
}

void pace_thread_pool_shutdown(void) {
    pthread_mutex_lock(&tp_mutex);
    tp_shutdown = 1;
    pthread_cond_broadcast(&tp_cond);
    pthread_mutex_unlock(&tp_mutex);
    
    for (size_t i = 0; i < num_workers; i++) {
        pthread_join(workers[i], NULL);
    }
    free(workers);
}

