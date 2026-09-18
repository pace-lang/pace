#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#include <stddef.h>

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

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
