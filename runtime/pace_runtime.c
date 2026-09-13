#include "pace_runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>

void PACE_RETAIN(long long obj) {
    // Basic ARC stub for MVP
}

void PACE_RELEASE(long long obj) {
    // Basic ARC stub for MVP
}

void pace_println() {
    printf("\n");
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
