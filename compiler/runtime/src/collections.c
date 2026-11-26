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

void pace_list_insert(PaceList* list, int64_t index, void* val) {
    if (!list) return;
    if (index < 0 || (size_t)index > list->length) {
        pace_panic("List insert index out of bounds");
    }
    if (list->length >= list->capacity) {
        size_t new_cap = list->capacity < 4 ? 4 : list->capacity * 2;
        void** new_data = (void**)realloc(list->data, new_cap * sizeof(void*));
        if (!new_data) {
            pace_panic("Fatal error: out of memory in pace_list_insert");
        }
        list->data = new_data;
        list->capacity = new_cap;
    }
    for (size_t i = list->length; i > (size_t)index; i--) {
        list->data[i] = list->data[i - 1];
    }
    if (list->elem_deinit && val) {
        pace_retain(val);
    }
    list->data[(size_t)index] = val;
    list->length++;
}

void* pace_list_remove_at(PaceList* list, int64_t index) {
    if (!list) return NULL;
    if (index < 0 || (size_t)index >= list->length) {
        pace_panic("List removeAt index out of bounds");
    }
    void* removed = list->data[(size_t)index];
    for (size_t i = (size_t)index; i + 1 < list->length; i++) {
        list->data[i] = list->data[i + 1];
    }
    list->length--;
    return removed;
}

int64_t pace_list_index_of_int(PaceList* list, int64_t val) {
    if (!list) return -1;
    for (size_t i = 0; i < list->length; i++) {
        if ((int64_t)(intptr_t)list->data[i] == val) return (int64_t)i;
    }
    return -1;
}

int64_t pace_list_index_of_str(PaceList* list, struct PaceString* val) {
    if (!list) return -1;
    for (size_t i = 0; i < list->length; i++) {
        if (pace_string_equals((struct PaceString*)list->data[i], val)) return (int64_t)i;
    }
    return -1;
}

int64_t pace_list_index_of_ptr(PaceList* list, void* val) {
    if (!list) return -1;
    for (size_t i = 0; i < list->length; i++) {
        if (list->data[i] == val) return (int64_t)i;
    }
    return -1;
}

int pace_list_contains_int(PaceList* list, int64_t val) {
    return pace_list_index_of_int(list, val) >= 0;
}

int pace_list_contains_str(PaceList* list, struct PaceString* val) {
    return pace_list_index_of_str(list, val) >= 0;
}

int pace_list_contains_ptr(PaceList* list, void* val) {
    return pace_list_index_of_ptr(list, val) >= 0;
}

PaceList* pace_list_slice(PaceList* list, int64_t start, int64_t end) {
    if (!list) return pace_list_new(4, NULL);
    int64_t len = (int64_t)list->length;
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (end < start) end = start;
    size_t count = (size_t)(end - start);
    PaceList* out = pace_list_new(count < 4 ? 4 : count, list->elem_deinit);
    for (size_t i = (size_t)start; i < (size_t)end; i++) {
        pace_list_push(out, list->data[i]);
    }
    return out;
}

PaceList* pace_list_reversed(PaceList* list) {
    if (!list) return pace_list_new(4, NULL);
    PaceList* out = pace_list_new(list->length < 4 ? 4 : list->length, list->elem_deinit);
    for (size_t i = list->length; i > 0; i--) {
        pace_list_push(out, list->data[i - 1]);
    }
    return out;
}

struct PaceString* pace_list_join(PaceList* list, struct PaceString* sep) {
    if (!list || list->length == 0) return pace_string_new("");
    size_t sep_len = sep ? sep->length : 0;
    size_t total = 0;
    for (size_t i = 0; i < list->length; i++) {
        struct PaceString* s = (struct PaceString*)list->data[i];
        if (s) total += s->length;
        if (i + 1 < list->length) total += sep_len;
    }
    char* buf = (char*)malloc(total + 1);
    if (!buf) pace_panic("Fatal error: out of memory in pace_list_join");
    size_t pos = 0;
    for (size_t i = 0; i < list->length; i++) {
        struct PaceString* s = (struct PaceString*)list->data[i];
        if (s && s->length > 0) {
            memcpy(buf + pos, s->data, s->length);
            pos += s->length;
        }
        if (i + 1 < list->length && sep && sep_len > 0) {
            memcpy(buf + pos, sep->data, sep_len);
            pos += sep_len;
        }
    }
    buf[pos] = '\0';
    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return res;
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


PaceList* pace_map_keys(PaceMap* map) {
    if (!map) return pace_list_new(4, NULL);
    void (*deinit)(void*) = map->key_is_string ? pace_release : NULL;
    PaceList* out = pace_list_new(map->count < 4 ? 4 : map->count, deinit);
    for (size_t i = 0; i < map->capacity; i++) {
        if (map->entries[i].state == MAP_STATE_OCCUPIED) {
            pace_list_push(out, map->entries[i].key);
        }
    }
    return out;
}

PaceList* pace_map_values(PaceMap* map) {
    if (!map) return pace_list_new(4, NULL);
    PaceList* out = pace_list_new(map->count < 4 ? 4 : map->count, map->val_deinit);
    for (size_t i = 0; i < map->capacity; i++) {
        if (map->entries[i].state == MAP_STATE_OCCUPIED) {
            pace_list_push(out, map->entries[i].val);
        }
    }
    return out;
}

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
                if (is_str) pace_set_add_str(out, (struct PaceString*)a->entries[i].key);
                else pace_set_add_int(out, (int64_t)(intptr_t)a->entries[i].key);
            }
        }
    }
    if (b) {
        for (size_t i = 0; i < b->capacity; i++) {
            if (b->entries[i].state == MAP_STATE_OCCUPIED) {
                if (is_str) pace_set_add_str(out, (struct PaceString*)b->entries[i].key);
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
            if (is_str) {
                struct PaceString* k = (struct PaceString*)a->entries[i].key;
                if (pace_set_has_str(b, k)) pace_set_add_str(out, k);
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
            if (is_str) {
                struct PaceString* k = (struct PaceString*)a->entries[i].key;
                if (!b || !pace_set_has_str(b, k)) pace_set_add_str(out, k);
            } else {
                int64_t k = (int64_t)(intptr_t)a->entries[i].key;
                if (!b || !pace_set_has_int(b, k)) pace_set_add_int(out, k);
            }
        }
    }
    return out;
}
