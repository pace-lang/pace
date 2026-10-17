#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "../pace_runtime.h"

// ---------------------------------------------------------
// PaceList Implementation
// ---------------------------------------------------------

typedef struct PaceList {
    void** data;
    size_t length;
    size_t capacity;
    void (*elem_deinit)(void*);
} PaceList;

static void pace_list_deinit(void* obj) {
    if (!obj) return;
    PaceList* list = (PaceList*)obj;
    if (list->data) {
        if (list->elem_deinit) {
            for (size_t i = 0; i < list->length; i++) {
                if (list->data[i]) {
                    list->elem_deinit(list->data[i]);
                }
            }
        }
        free(list->data);
        list->data = NULL;
    }
}

PaceList* pace_list_new(size_t capacity, void (*elem_deinit)(void*)) {
    PaceList* list = (PaceList*)pace_alloc(sizeof(PaceList), pace_list_deinit);
    if (capacity < 4) capacity = 4;
    list->data = (void**)malloc(capacity * sizeof(void*));
    if (!list->data) {
        pace_panic("Fatal error: out of memory in pace_list_new");
    }
    list->length = 0;
    list->capacity = capacity;
    list->elem_deinit = elem_deinit;
    return list;
}

void pace_list_push(PaceList* list, void* val) {
    if (!list) return;
    if (list->length >= list->capacity) {
        size_t new_cap = list->capacity < 4 ? 4 : list->capacity * 2;
        void** new_data = (void**)realloc(list->data, new_cap * sizeof(void*));
        if (!new_data) {
            pace_panic("Fatal error: out of memory in pace_list_push");
        }
        list->data = new_data;
        list->capacity = new_cap;
    }
    if (list->elem_deinit && val) {
        pace_retain(val);
    }
    list->data[list->length++] = val;
}

void* pace_list_pop(PaceList* list) {
    if (!list || list->length == 0) return NULL;
    return list->data[--list->length];
}

void* pace_list_get(PaceList* list, int64_t index) {
    if (!list) return NULL;
    if (index < 0 || (size_t)index >= list->length) {
        pace_panic("List index out of bounds");
    }
    return list->data[(size_t)index];
}

void pace_list_set(PaceList* list, int64_t index, void* val) {
    if (!list) return;
    if (index < 0 || (size_t)index >= list->length) {
        pace_panic("List index out of bounds");
    }
    void* old = list->data[(size_t)index];
    if (list->elem_deinit) {
        if (val) pace_retain(val);
        list->data[(size_t)index] = val;
        if (old) list->elem_deinit(old);
    } else {
        list->data[(size_t)index] = val;
    }
}

int64_t pace_list_len(PaceList* list) {
    if (!list) return 0;
    return (int64_t)list->length;
}

void pace_list_clear(PaceList* list) {
    if (!list) return;
    if (list->elem_deinit) {
        for (size_t i = 0; i < list->length; i++) {
            if (list->data[i]) {
                list->elem_deinit(list->data[i]);
            }
        }
    }
    list->length = 0;
}

// ---------------------------------------------------------
// PaceMap Implementation
// ---------------------------------------------------------

#define MAP_STATE_EMPTY 0
#define MAP_STATE_OCCUPIED 1
#define MAP_STATE_TOMBSTONE 2

typedef struct PaceMapEntry {
    uint64_t hash;
    void* key;
    void* val;
    uint8_t state;
} PaceMapEntry;

typedef struct PaceMap {
    PaceMapEntry* entries;
    size_t capacity;
    size_t count;
    size_t tombstones;
    int key_is_string;
    void (*val_deinit)(void*);
} PaceMap;

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

static void pace_map_deinit(void* obj) {
    if (!obj) return;
    PaceMap* map = (PaceMap*)obj;
    if (map->entries) {
        for (size_t i = 0; i < map->capacity; i++) {
            if (map->entries[i].state == MAP_STATE_OCCUPIED) {
                if (map->key_is_string && map->entries[i].key) {
                    pace_release(map->entries[i].key);
                }
                if (map->val_deinit && map->entries[i].val) {
                    map->val_deinit(map->entries[i].val);
                }
            }
        }
        free(map->entries);
        map->entries = NULL;
    }
}

static void pace_map_resize(PaceMap* map, size_t new_cap) {
    PaceMapEntry* old_entries = map->entries;
    size_t old_cap = map->capacity;

    PaceMapEntry* new_entries = (PaceMapEntry*)calloc(new_cap, sizeof(PaceMapEntry));
    if (!new_entries) {
        pace_panic("Fatal error: out of memory in pace_map_resize");
    }

    map->entries = new_entries;
    map->capacity = new_cap;
    map->count = 0;
    map->tombstones = 0;

    uint64_t mask = (uint64_t)(new_cap - 1);
    for (size_t i = 0; i < old_cap; i++) {
        if (old_entries[i].state == MAP_STATE_OCCUPIED) {
            uint64_t h = old_entries[i].hash;
            size_t idx = (size_t)(h & mask);
            while (map->entries[idx].state == MAP_STATE_OCCUPIED) {
                idx = (idx + 1) & mask;
            }
            map->entries[idx] = old_entries[i];
            map->count++;
        }
    }

    free(old_entries);
}

PaceMap* pace_map_new(size_t capacity, int key_is_string, void (*val_deinit)(void*)) {
    PaceMap* map = (PaceMap*)pace_alloc(sizeof(PaceMap), pace_map_deinit);
    size_t cap = 16;
    while (cap < capacity) cap *= 2;
    map->entries = (PaceMapEntry*)calloc(cap, sizeof(PaceMapEntry));
    if (!map->entries) {
        pace_panic("Fatal error: out of memory in pace_map_new");
    }
    map->capacity = cap;
    map->count = 0;
    map->tombstones = 0;
    map->key_is_string = key_is_string;
    map->val_deinit = val_deinit;
    return map;
}

void pace_map_set_int(PaceMap* map, int64_t key, void* val) {
    if (!map) return;
    if ((map->count + map->tombstones + 1) * 10 >= map->capacity * 7) {
        pace_map_resize(map, map->capacity * 2);
    }

    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);
    int64_t first_tombstone = -1;

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            size_t target_idx = (first_tombstone != -1) ? (size_t)first_tombstone : idx;
            if (first_tombstone != -1) {
                map->tombstones--;
            }
            if (map->val_deinit && val) pace_retain(val);
            map->entries[target_idx].hash = h;
            map->entries[target_idx].key = (void*)(intptr_t)key;
            map->entries[target_idx].val = val;
            map->entries[target_idx].state = MAP_STATE_OCCUPIED;
            map->count++;
            return;
        } else if (e->state == MAP_STATE_TOMBSTONE) {
            if (first_tombstone == -1) {
                first_tombstone = (int64_t)idx;
            }
        } else if (e->state == MAP_STATE_OCCUPIED && e->hash == h && (int64_t)(intptr_t)e->key == key) {
            void* old = e->val;
            if (map->val_deinit && val) pace_retain(val);
            e->val = val;
            if (map->val_deinit && old) map->val_deinit(old);
            return;
        }
        idx = (idx + 1) & mask;
    }
}

void* pace_map_get_int(PaceMap* map, int64_t key, int* found) {
    if (!map || map->count == 0) {
        if (found) *found = 0;
        return NULL;
    }
    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            if (found) *found = 0;
            return NULL;
        }
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && (int64_t)(intptr_t)e->key == key) {
            if (found) *found = 1;
            return e->val;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_map_has_int(PaceMap* map, int64_t key) {
    int found = 0;
    pace_map_get_int(map, key, &found);
    return found;
}

int pace_map_remove_int(PaceMap* map, int64_t key) {
    if (!map || map->count == 0) return 0;
    uint64_t h = hash_int(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && (int64_t)(intptr_t)e->key == key) {
            e->state = MAP_STATE_TOMBSTONE;
            if (map->val_deinit && e->val) {
                map->val_deinit(e->val);
            }
            e->val = NULL;
            map->count--;
            map->tombstones++;
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

void pace_map_set_str(PaceMap* map, struct PaceString* key, void* val) {
    if (!map || !key) return;
    if ((map->count + map->tombstones + 1) * 10 >= map->capacity * 7) {
        pace_map_resize(map, map->capacity * 2);
    }

    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);
    int64_t first_tombstone = -1;

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            size_t target_idx = (first_tombstone != -1) ? (size_t)first_tombstone : idx;
            if (first_tombstone != -1) {
                map->tombstones--;
            }
            pace_retain(key);
            if (map->val_deinit && val) pace_retain(val);
            map->entries[target_idx].hash = h;
            map->entries[target_idx].key = key;
            map->entries[target_idx].val = val;
            map->entries[target_idx].state = MAP_STATE_OCCUPIED;
            map->count++;
            return;
        } else if (e->state == MAP_STATE_TOMBSTONE) {
            if (first_tombstone == -1) {
                first_tombstone = (int64_t)idx;
            }
        } else if (e->state == MAP_STATE_OCCUPIED && e->hash == h && key_str_eq((struct PaceString*)e->key, key)) {
            void* old = e->val;
            if (map->val_deinit && val) pace_retain(val);
            e->val = val;
            if (map->val_deinit && old) map->val_deinit(old);
            return;
        }
        idx = (idx + 1) & mask;
    }
}

void* pace_map_get_str(PaceMap* map, struct PaceString* key, int* found) {
    if (!map || !key || map->count == 0) {
        if (found) *found = 0;
        return NULL;
    }
    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) {
            if (found) *found = 0;
            return NULL;
        }
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && key_str_eq((struct PaceString*)e->key, key)) {
            if (found) *found = 1;
            return e->val;
        }
        idx = (idx + 1) & mask;
    }
}

int pace_map_has_str(PaceMap* map, struct PaceString* key) {
    int found = 0;
    pace_map_get_str(map, key, &found);
    return found;
}

int pace_map_remove_str(PaceMap* map, struct PaceString* key) {
    if (!map || !key || map->count == 0) return 0;
    uint64_t h = hash_str(key);
    uint64_t mask = (uint64_t)(map->capacity - 1);
    size_t idx = (size_t)(h & mask);

    while (1) {
        PaceMapEntry* e = &map->entries[idx];
        if (e->state == MAP_STATE_EMPTY) return 0;
        if (e->state == MAP_STATE_OCCUPIED && e->hash == h && key_str_eq((struct PaceString*)e->key, key)) {
            e->state = MAP_STATE_TOMBSTONE;
            pace_release(e->key);
            e->key = NULL;
            if (map->val_deinit && e->val) {
                map->val_deinit(e->val);
            }
            e->val = NULL;
            map->count--;
            map->tombstones++;
            return 1;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t pace_map_len(PaceMap* map) {
    if (!map) return 0;
    return (int64_t)map->count;
}

void pace_map_clear(PaceMap* map) {
    if (!map) return;
    for (size_t i = 0; i < map->capacity; i++) {
        if (map->entries[i].state == MAP_STATE_OCCUPIED) {
            if (map->key_is_string && map->entries[i].key) {
                pace_release(map->entries[i].key);
            }
            if (map->val_deinit && map->entries[i].val) {
                map->val_deinit(map->entries[i].val);
            }
        }
        map->entries[i].state = MAP_STATE_EMPTY;
    }
    map->count = 0;
    map->tombstones = 0;
}
