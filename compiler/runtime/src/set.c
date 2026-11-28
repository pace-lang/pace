#include "collections_common.h"

// ---------------------------------------------------------
// PaceSet Implementation (Open-Addressing Hash Set)
// ---------------------------------------------------------

typedef struct PaceSetEntry {
    uint64_t hash;
    void* key;
    uint8_t state;
} PaceSetEntry;

typedef struct PaceSet {
    PaceSetEntry* entries;
    size_t capacity;
    size_t count;
    size_t tombstones;
    int key_is_string;
} PaceSet;

static void pace_set_deinit(void* obj) {
    if (!obj) return;
    PaceSet* set = (PaceSet*)obj;
    if (set->entries) {
        if (set->key_is_string) {
            for (size_t i = 0; i < set->capacity; i++) {
                if (set->entries[i].state == MAP_STATE_OCCUPIED && set->entries[i].key) {
                    pace_release(set->entries[i].key);
                }
            }
        }
        free(set->entries);
        set->entries = NULL;
    }
}

static void pace_set_resize(PaceSet* set, size_t new_cap) {
    if (new_cap < 16) new_cap = 16;
    PaceSetEntry* new_entries = (PaceSetEntry*)calloc(new_cap, sizeof(PaceSetEntry));
    if (!new_entries) pace_panic("Fatal error: out of memory in pace_set_resize");
    uint64_t mask = (uint64_t)(new_cap - 1);
    for (size_t i = 0; i < set->capacity; i++) {
        if (set->entries[i].state == MAP_STATE_OCCUPIED) {
            size_t idx = (size_t)(set->entries[i].hash & mask);
            while (new_entries[idx].state == MAP_STATE_OCCUPIED) {
                idx = (idx + 1) & mask;
            }
            new_entries[idx] = set->entries[i];
        }
    }
    free(set->entries);
    set->entries = new_entries;
    set->capacity = new_cap;
    set->tombstones = 0;
}

PaceSet* pace_set_new(size_t capacity, int key_is_string) {
    PaceSet* set = (PaceSet*)pace_alloc(sizeof(PaceSet), pace_set_deinit);
    size_t cap = 16;
    while (cap < capacity) cap *= 2;
    set->entries = (PaceSetEntry*)calloc(cap, sizeof(PaceSetEntry));
    if (!set->entries) pace_panic("Fatal error: out of memory in pace_set_new");
    set->capacity = cap;
    set->count = 0;
    set->tombstones = 0;
    set->key_is_string = key_is_string;
    return set;
}

int pace_set_add_int(PaceSet* set, int64_t key) {
    if (!set) return 0;
    if ((set->count + set->tombstones + 1) * 10 >= set->capacity * 7) {
        pace_set_resize(set, set->capacity * 2);
    }
    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    size_t first_tomb = (size_t)-1;
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            size_t target = (first_tomb != (size_t)-1) ? first_tomb : idx;
            if (first_tomb != (size_t)-1) set->tombstones--;
            set->entries[target].hash = h;
            set->entries[target].key = (void*)(intptr_t)key;
            set->entries[target].state = MAP_STATE_OCCUPIED;
            set->count++;
            return 1;
        }
        if (e->state == MAP_STATE_TOMBSTONE) {
            if (first_tomb == (size_t)-1) first_tomb = idx;
        } else if (e->hash == h && (int64_t)(intptr_t)e->key == key) {
            return 0;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_set_has_int(PaceSet* set, int64_t key) {
    if (!set || set->count == 0) return 0;
    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && (int64_t)(intptr_t)e->key == key) return 1;
        idx = (idx + 1) & mask;
    }
}

int pace_set_remove_int(PaceSet* set, int64_t key) {
    if (!set || set->count == 0) return 0;
    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && (int64_t)(intptr_t)e->key == key) {
            e->state = MAP_STATE_TOMBSTONE;
            e->key = NULL;
            set->count--;
            set->tombstones++;
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_set_add_str(PaceSet* set, struct PaceString* key) {
    if (!set || !key) return 0;
    if ((set->count + set->tombstones + 1) * 10 >= set->capacity * 7) {
        pace_set_resize(set, set->capacity * 2);
    }
    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    size_t first_tomb = (size_t)-1;
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            size_t target = (first_tomb != (size_t)-1) ? first_tomb : idx;
            if (first_tomb != (size_t)-1) set->tombstones--;
            pace_retain(key);
            set->entries[target].hash = h;
            set->entries[target].key = key;
            set->entries[target].state = MAP_STATE_OCCUPIED;
            set->count++;
            return 1;
        }
        if (e->state == MAP_STATE_TOMBSTONE) {
            if (first_tomb == (size_t)-1) first_tomb = idx;
        } else if (e->hash == h && key_str_eq((struct PaceString*)e->key, key)) {
            return 0;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_set_has_str(PaceSet* set, struct PaceString* key) {
    if (!set || !key || set->count == 0) return 0;
    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && key_str_eq((struct PaceString*)e->key, key)) return 1;
        idx = (idx + 1) & mask;
    }
}

int pace_set_remove_str(PaceSet* set, struct PaceString* key) {
    if (!set || !key || set->count == 0) return 0;
    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && key_str_eq((struct PaceString*)e->key, key)) {
            e->state = MAP_STATE_TOMBSTONE;
            pace_release(e->key);
            e->key = NULL;
            set->count--;
            set->tombstones++;
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

static uint64_t hash_float(double key) {
    if (key == 0.0) key = 0.0; // normalize -0.0
    uint64_t bits = 0;
    memcpy(&bits, &key, sizeof(double));
    return hash_int((int64_t)bits);
}

int pace_set_add_float(PaceSet* set, double key) {
    if (!set) return 0;
    if ((set->count + set->tombstones + 1) * 10 >= set->capacity * 7) {
        pace_set_resize(set, set->capacity * 2);
    }
    if (key == 0.0) key = 0.0;
    uint64_t h = hash_float(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    size_t first_tomb = (size_t)-1;
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            size_t target = (first_tomb != (size_t)-1) ? first_tomb : idx;
            if (first_tomb != (size_t)-1) set->tombstones--;
            double* boxed = (double*)pace_alloc(sizeof(double), NULL);
            *boxed = key;
            set->entries[target].hash = h;
            set->entries[target].key = boxed;
            set->entries[target].state = MAP_STATE_OCCUPIED;
            set->count++;
            return 1;
        }
        if (e->state == MAP_STATE_TOMBSTONE) {
            if (first_tomb == (size_t)-1) first_tomb = idx;
        } else if (e->hash == h && *(double*)e->key == key) {
            return 0;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_set_has_float(PaceSet* set, double key) {
    if (!set || set->count == 0) return 0;
    if (key == 0.0) key = 0.0;
    uint64_t h = hash_float(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && *(double*)e->key == key) return 1;
        idx = (idx + 1) & mask;
    }
}

int pace_set_remove_float(PaceSet* set, double key) {
    if (!set || set->count == 0) return 0;
    if (key == 0.0) key = 0.0;
    uint64_t h = hash_float(key);
    uint64_t mask = (uint64_t)(set->capacity - 1);
    size_t idx = (size_t)(h & mask);
    while (1) {
        PaceSetEntry* e = &set->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && *(double*)e->key == key) {
            e->state = MAP_STATE_TOMBSTONE;
            pace_release(e->key);
            e->key = NULL;
            set->count--;
            set->tombstones++;
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t pace_set_len(PaceSet* set) {
    if (!set) return 0;
    return (int64_t)set->count;
}

void pace_set_clear(PaceSet* set) {
    if (!set) return;
    if (set->key_is_string) {
        for (size_t i = 0; i < set->capacity; i++) {
            if (set->entries[i].state == MAP_STATE_OCCUPIED && set->entries[i].key) {
                pace_release(set->entries[i].key);
            }
        }
    }
    memset(set->entries, 0, set->capacity * sizeof(PaceSetEntry));
    set->count = 0;
    set->tombstones = 0;
}

PaceList* pace_set_to_list(PaceSet* set) {
    if (!set) return pace_list_new(4, NULL);
    void (*deinit)(void*) = set->key_is_string ? pace_release : NULL;
    PaceList* out = pace_list_new(set->count < 4 ? 4 : set->count, deinit);
    for (size_t i = 0; i < set->capacity; i++) {
        if (set->entries[i].state == MAP_STATE_OCCUPIED) {
            pace_list_push(out, set->entries[i].key);
        }
    }
    return out;
}

PaceSet* pace_set_union(PaceSet* a, PaceSet* b) {
    int is_str = a ? a->key_is_string : (b ? b->key_is_string : 0);
    PaceSet* out = pace_set_new(16, is_str);
    if (a) {
        for (size_t i = 0; i < a->capacity; i++) {
            if (a->entries[i].state == MAP_STATE_OCCUPIED) {
                if (is_str == 1) pace_set_add_str(out, (struct PaceString*)a->entries[i].key);
                else if (is_str == 2) pace_set_add_float(out, *(double*)a->entries[i].key);
                else pace_set_add_int(out, (int64_t)(intptr_t)a->entries[i].key);
            }
        }
    }
    if (b) {
        for (size_t i = 0; i < b->capacity; i++) {
            if (b->entries[i].state == MAP_STATE_OCCUPIED) {
                if (is_str == 1) pace_set_add_str(out, (struct PaceString*)b->entries[i].key);
                else if (is_str == 2) pace_set_add_float(out, *(double*)b->entries[i].key);
                else pace_set_add_int(out, (int64_t)(intptr_t)b->entries[i].key);
            }
        }
    }
    return out;
}

PaceSet* pace_set_intersection(PaceSet* a, PaceSet* b) {
    int is_str = a ? a->key_is_string : 0;
    PaceSet* out = pace_set_new(16, is_str);
    if (!a || !b) return out;
    for (size_t i = 0; i < a->capacity; i++) {
        if (a->entries[i].state == MAP_STATE_OCCUPIED) {
            if (is_str == 1) {
                struct PaceString* k = (struct PaceString*)a->entries[i].key;
                if (pace_set_has_str(b, k)) pace_set_add_str(out, k);
            } else if (is_str == 2) {
                double k = *(double*)a->entries[i].key;
                if (pace_set_has_float(b, k)) pace_set_add_float(out, k);
            } else {
                int64_t k = (int64_t)(intptr_t)a->entries[i].key;
                if (pace_set_has_int(b, k)) pace_set_add_int(out, k);
            }
        }
    }
    return out;
}

PaceSet* pace_set_difference(PaceSet* a, PaceSet* b) {
    int is_str = a ? a->key_is_string : 0;
    PaceSet* out = pace_set_new(16, is_str);
    if (!a) return out;
    for (size_t i = 0; i < a->capacity; i++) {
        if (a->entries[i].state == MAP_STATE_OCCUPIED) {
            if (is_str == 1) {
                struct PaceString* k = (struct PaceString*)a->entries[i].key;
                if (!b || !pace_set_has_str(b, k)) pace_set_add_str(out, k);
            } else if (is_str == 2) {
                double k = *(double*)a->entries[i].key;
                if (!b || !pace_set_has_float(b, k)) pace_set_add_float(out, k);
            } else {
                int64_t k = (int64_t)(intptr_t)a->entries[i].key;
                if (!b || !pace_set_has_int(b, k)) pace_set_add_int(out, k);
            }
        }
    }
    return out;
}
