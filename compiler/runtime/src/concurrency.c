#include <stdlib.h>
#include <pthread.h>

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
