#ifndef PACE_COLLECTIONS_COMMON_H
#define PACE_COLLECTIONS_COMMON_H

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "../pace_runtime.h"

#define MAP_STATE_EMPTY 0
#define MAP_STATE_OCCUPIED 1
#define MAP_STATE_TOMBSTONE 2


static inline uint64_t hash_int(int64_t x) {
    uint64_t z = (uint64_t)x;
    z = (z ^ (z >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)) * UINT64_C(0x94d049bb133111eb);
    return z ^ (z >> 31);
}

static inline uint64_t hash_str(struct PaceString* str) {
    if (!str || !str->data) return 0;
    uint64_t h = UINT64_C(14695981039346656037);
    const uint8_t* p = (const uint8_t*)str->data;
    for (size_t i = 0; i < str->length; i++) {
        h ^= (uint64_t)p[i];
        h *= UINT64_C(1099511628211);
    }
    return h;
}

static inline int key_str_eq(struct PaceString* a, struct PaceString* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (a->length != b->length) return 0;
    return memcmp(a->data, b->data, a->length) == 0;
}

#endif // PACE_COLLECTIONS_COMMON_H
