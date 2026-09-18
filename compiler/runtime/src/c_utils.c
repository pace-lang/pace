#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>

// Extern declaration for pace_string_new defined in Rust
extern void* pace_string_new(const char* c_str);

void* pace_format_string(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int size = vsnprintf(NULL, 0, fmt, args);
    va_end(args);
    
    char* buf = (char*)malloc(size + 1);
    if (!buf) {
        // Fallback or panic
        abort();
    }
    
    va_start(args, fmt);
    vsnprintf(buf, size + 1, fmt, args);
    va_end(args);
    
    void* pace_str = pace_string_new(buf);
    free(buf);
    
    return pace_str;
}
