#include "pace_runtime.h"
#include <stdio.h>
#include <stdlib.h>

void PACE_RETAIN(long long obj) {
    // Basic ARC stub for MVP
}

void PACE_RELEASE(long long obj) {
    // Basic ARC stub for MVP
}

void pace_println() {
    printf("\n");
}

struct pace_arc_header {
    long long ref_count;
    pace_destructor_t dtor;
};

void* pace_alloc(size_t size, pace_destructor_t dtor) {
    struct pace_arc_header* hdr = (struct pace_arc_header*)malloc(sizeof(struct pace_arc_header) + size);
    if (!hdr) return NULL;
    hdr->ref_count = 1;
    hdr->dtor = dtor;
    return (void*)(hdr + 1);
}

void pace_retain(void* obj) {
    if (!obj) return;
    struct pace_arc_header* hdr = ((struct pace_arc_header*)obj) - 1;
    hdr->ref_count++;
}

void pace_release(void* obj) {
    if (!obj) return;
    struct pace_arc_header* hdr = ((struct pace_arc_header*)obj) - 1;
    hdr->ref_count--;
    if (hdr->ref_count == 0) {
        if (hdr->dtor) {
            hdr->dtor(obj);
        }
        free(hdr);
    }
}

void pace_print_int(long long val) {
    printf("%lld\n", val);
}

void pace_print_string(const char* val) {
    printf("%s\n", val);
}
