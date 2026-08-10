#include <stdio.h>
#include <stdint.h>

#include <stdlib.h>
#include <string.h>

typedef struct {
    uint64_t field_count;
    uint64_t field_offsets[];
} PaceClassMetadata;

int64_t pace_print(int64_t value) {
    printf("%lld\n", (long long)value);
    return 0;
}

void* pace_alloc(int64_t size, void* metadata_ptr) {
    // Allocate and zero memory
    void* ptr = calloc(1, size);
    if (!ptr) {
        printf("Pace Runtime Error: Out of memory\n");
        exit(1);
    }
    // Set strong reference count to 1 (offset 0)
    *(uint64_t*)ptr = 1;
    // Set weak reference count to 1 (offset 8)
    // The strong count's existence contributes 1 to the weak count.
    // When strong_count reaches 0, it calls pace_weak_release().
    *(uint64_t*)((char*)ptr + 8) = 1;
    
    // Offset 16 is type metadata
    *(uint64_t*)((char*)ptr + 16) = (uint64_t)metadata_ptr;
    
    return ptr;
}

void* pace_alloc_array_repeat(uint64_t count, uint64_t val, uint64_t metadata_val) {
    uint64_t total_size = 24 + count * 8;
    void* ptr = pace_alloc(total_size, (void*)metadata_val);
    *(uint64_t*)((char*)ptr + 24) = count;
    for (uint64_t i = 0; i < count; i++) {
        *(uint64_t*)((char*)ptr + 32 + i * 8) = val;
    }
    return ptr;
}

void pace_retain(void* obj) {
    if (!obj) return;
    // Atomic increment of strong count at offset 0
    __atomic_add_fetch((uint64_t*)obj, 1, __ATOMIC_SEQ_CST);
}

void pace_weak_release(void* obj) {
    if (!obj) return;
    uint64_t* weak_count_ptr = (uint64_t*)((char*)obj + 8);
    uint64_t new_count = __atomic_sub_fetch(weak_count_ptr, 1, __ATOMIC_SEQ_CST);
    if (new_count == 0) {
        // Free the memory allocation
        printf("Pace Runtime: Freeing object memory\n");
        free(obj);
    }
}

void pace_release(void* obj) {
    if (!obj) return;
    // Atomic decrement of strong count at offset 0
    uint64_t new_count = __atomic_sub_fetch((uint64_t*)obj, 1, __ATOMIC_SEQ_CST);
    if (new_count == 0) {
        printf("Pace Runtime: Strong count is 0, releasing weak representation\n");
        
        uint64_t metadata_val = *(uint64_t*)((char*)obj + 16);
        if (metadata_val == (uint64_t)-1) { // Array of references
            uint64_t length = *(uint64_t*)((char*)obj + 24);
            for (uint64_t i = 0; i < length; i++) {
                void* element = (void*)(*(uint64_t*)((char*)obj + 32 + i * 8));
                if (element) {
                    pace_release(element);
                }
            }
        } else if (metadata_val != (uint64_t)-2 && metadata_val != 0) { // -2 is Array of primitives
            PaceClassMetadata* metadata = (PaceClassMetadata*)metadata_val;
            for (uint64_t i = 0; i < metadata->field_count; i++) {
                uint64_t offset = metadata->field_offsets[i];
                void* field_ptr = (void*)(*(uint64_t*)((char*)obj + offset));
                if (field_ptr) {
                    pace_release(field_ptr);
                }
            }
        }
        
        // Drop the weak count that was held on behalf of the strong count
        pace_weak_release(obj);
    }
}

void pace_weak_retain(void* obj) {
    if (!obj) return;
    uint64_t* weak_count_ptr = (uint64_t*)((char*)obj + 8);
    __atomic_add_fetch(weak_count_ptr, 1, __ATOMIC_SEQ_CST);
}

void* pace_weak_upgrade(void* obj) {
    if (!obj) return NULL;
    uint64_t* strong_count_ptr = (uint64_t*)obj;
    uint64_t count = __atomic_load_n(strong_count_ptr, __ATOMIC_SEQ_CST);
    while (count > 0) {
        if (__atomic_compare_exchange_n(strong_count_ptr, &count, count + 1, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) {
            return obj;
        }
    }
    return NULL;
}
