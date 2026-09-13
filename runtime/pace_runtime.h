#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void PACE_RETAIN(long long obj);
void PACE_RELEASE(long long obj);

void pace_print_int(long long val);
void pace_print_float(double val);
void pace_print_bool(int val);
void pace_print_string(const char* val);
void pace_println();
char* pace_format_string(const char* fmt, ...);

// ARC object header
typedef struct {
    int ref_count;
    void (*deinit)(void*); // Destructor to recursively release children
} pace_arc_header;

typedef void (*pace_destructor_t)(void*);

void* pace_alloc(size_t size, void (*deinit)(void*));
void pace_retain(void* obj);
void pace_release(void* obj);

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
