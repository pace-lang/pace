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

