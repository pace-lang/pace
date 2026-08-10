#include <stdio.h>
#include <stdint.h>

#include <stdlib.h>
#include <string.h>

int64_t pace_print(int64_t value) {
    printf("%lld\n", (long long)value);
    return 0;
}

void* pace_alloc(int64_t size) {
    // Allocate and zero memory
    void* ptr = calloc(1, size);
    if (!ptr) {
        printf("Pace Runtime Error: Out of memory\n");
        exit(1);
    }
    // Set reference count to 1 (offset 0)
    *(uint64_t*)ptr = 1;
    // Offset 8 is type metadata (placeholder, 0 for now)
    return ptr;
}

void pace_retain(void* obj) {
    if (!obj) return;
    // Atomic increment of reference count at offset 0
    __atomic_add_fetch((uint64_t*)obj, 1, __ATOMIC_SEQ_CST);
}

void pace_release(void* obj) {
    if (!obj) return;
    // Atomic decrement of reference count at offset 0
    uint64_t new_count = __atomic_sub_fetch((uint64_t*)obj, 1, __ATOMIC_SEQ_CST);
    if (new_count == 0) {
        // Free the object
        printf("Pace Runtime: Freeing object\n");
        free(obj);
    }
}
