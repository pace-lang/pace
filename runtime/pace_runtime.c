#include "pace_runtime.h"
#include <stdio.h>
#include <stdlib.h>

void PACE_RETAIN(long long obj) {
    // Basic ARC stub for MVP
}

void PACE_RELEASE(long long obj) {
    // Basic ARC stub for MVP
}

void pace_print_int(long long val) {
    printf("%lld\n", val);
}

void pace_print_string(const char* val) {
    printf("%s\n", val);
}
