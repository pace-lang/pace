#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

// Forward declarations of runtime functions
void* pace_alloc(int64_t size, void* metadata_ptr);
void pace_retain(void* obj);
void pace_release(void* obj);

typedef struct {
    uint64_t field_count;
    uint64_t field_offsets[];
} PaceClassMetadata;

PaceClassMetadata empty_metadata = {0};

void* shared_obj = NULL;

void* thread_func(void* arg) {
    for (int i = 0; i < 10000; i++) {
        pace_retain(shared_obj);
    }
    for (int i = 0; i < 10000; i++) {
        pace_release(shared_obj);
    }
    return NULL;
}

int main() {
    // 24 bytes header + 8 bytes padding
    shared_obj = pace_alloc(32, &empty_metadata);
    
    pthread_t t1, t2, t3, t4;
    
    pthread_create(&t1, NULL, thread_func, NULL);
    pthread_create(&t2, NULL, thread_func, NULL);
    pthread_create(&t3, NULL, thread_func, NULL);
    pthread_create(&t4, NULL, thread_func, NULL);
    
    pthread_join(t1, NULL);
    pthread_join(t2, NULL);
    pthread_join(t3, NULL);
    pthread_join(t4, NULL);
    
    // Release the initial strong reference
    pace_release(shared_obj);
    
    // If we reach here without a segfault or double free, and the memory
    // log shows a single "Freeing object memory", then our atomic RC is safe!
    return 0;
}
