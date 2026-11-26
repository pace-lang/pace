#include <ctype.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "../pace_runtime.h"

static struct PaceString* pace_string_from_slice(const char* src, size_t len) {
    char* buf = (char*)malloc(len + 1);
    if (!buf) pace_panic("Fatal error: out of memory in pace_string_from_slice");
    if (len > 0) memcpy(buf, src, len);
    buf[len] = '\0';
    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return res;
}

int64_t pace_string_len(struct PaceString* s) {
    if (!s) return 0;
    return (int64_t)s->length;
}

int64_t pace_string_index_of(struct PaceString* s, struct PaceString* sub) {
    if (!s || !sub) return -1;
    if (sub->length == 0) return 0;
    if (sub->length > s->length) return -1;
    const char* found = strstr(s->data, sub->data);
    if (!found) return -1;
    return (int64_t)(found - s->data);
}

int pace_string_contains(struct PaceString* s, struct PaceString* sub) {
    return pace_string_index_of(s, sub) >= 0;
}

int pace_string_starts_with(struct PaceString* s, struct PaceString* prefix) {
    if (!s || !prefix) return 0;
    if (prefix->length > s->length) return 0;
    return memcmp(s->data, prefix->data, prefix->length) == 0;
}

int pace_string_ends_with(struct PaceString* s, struct PaceString* suffix) {
    if (!s || !suffix) return 0;
    if (suffix->length > s->length) return 0;
    return memcmp(s->data + (s->length - suffix->length), suffix->data, suffix->length) == 0;
}

struct PaceString* pace_string_trim(struct PaceString* s) {
    if (!s || s->length == 0) return pace_string_new("");
    size_t start = 0;
    while (start < s->length && isspace((unsigned char)s->data[start])) start++;
    size_t end = s->length;
    while (end > start && isspace((unsigned char)s->data[end - 1])) end--;
    return pace_string_from_slice(s->data + start, end - start);
}

struct PaceString* pace_string_slice(struct PaceString* s, int64_t start, int64_t end) {
    if (!s || s->length == 0) return pace_string_new("");
    int64_t len = (int64_t)s->length;
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (end <= start) return pace_string_new("");
    return pace_string_from_slice(s->data + start, (size_t)(end - start));
}

struct PaceString* pace_string_to_lower(struct PaceString* s) {
    if (!s) return pace_string_new("");
    char* buf = (char*)malloc(s->length + 1);
    if (!buf) pace_panic("Fatal error: out of memory in pace_string_to_lower");
    for (size_t i = 0; i < s->length; i++) {
        buf[i] = (char)tolower((unsigned char)s->data[i]);
    }
    buf[s->length] = '\0';
    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return res;
}

struct PaceString* pace_string_to_upper(struct PaceString* s) {
    if (!s) return pace_string_new("");
    char* buf = (char*)malloc(s->length + 1);
    if (!buf) pace_panic("Fatal error: out of memory in pace_string_to_upper");
    for (size_t i = 0; i < s->length; i++) {
        buf[i] = (char)toupper((unsigned char)s->data[i]);
    }
    buf[s->length] = '\0';
    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return res;
}

struct PaceString* pace_string_replace(struct PaceString* s, struct PaceString* from, struct PaceString* to) {
    if (!s) return pace_string_new("");
    if (!from || from->length == 0 || !to) return pace_string_from_slice(s->data, s->length);

    size_t count = 0;
    const char* p = s->data;
    while ((p = strstr(p, from->data)) != NULL) {
        count++;
        p += from->length;
    }
    if (count == 0) return pace_string_from_slice(s->data, s->length);

    size_t new_len = s->length + count * (to->length - from->length);
    char* buf = (char*)malloc(new_len + 1);
    if (!buf) pace_panic("Fatal error: out of memory in pace_string_replace");

    char* dst = buf;
    p = s->data;
    const char* hit;
    while ((hit = strstr(p, from->data)) != NULL) {
        size_t chunk = (size_t)(hit - p);
        memcpy(dst, p, chunk);
        dst += chunk;
        if (to->length > 0) {
            memcpy(dst, to->data, to->length);
            dst += to->length;
        }
        p = hit + from->length;
    }
    size_t tail = strlen(p);
    memcpy(dst, p, tail);
    dst[tail] = '\0';

    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return res;
}

PaceList* pace_string_split(struct PaceString* s, struct PaceString* delim) {
    PaceList* out = pace_list_new(8, pace_release);
    if (!s) return out;
    if (!delim || delim->length == 0) {
        for (size_t i = 0; i < s->length; i++) {
            struct PaceString* ch = pace_string_from_slice(s->data + i, 1);
            pace_list_push(out, ch);
            pace_release(ch);
        }
        return out;
    }
    const char* p = s->data;
    const char* hit;
    while ((hit = strstr(p, delim->data)) != NULL) {
        struct PaceString* part = pace_string_from_slice(p, (size_t)(hit - p));
        pace_list_push(out, part);
        pace_release(part);
        p = hit + delim->length;
    }
    struct PaceString* tail = pace_string_new(p);
    pace_list_push(out, tail);
    pace_release(tail);
    return out;
}

// ---------------------------------------------------------
// PaceStringBuilder Implementation
// ---------------------------------------------------------

typedef struct PaceStringBuilder {
    char* buf;
    size_t length;
    size_t capacity;
} PaceStringBuilder;

static void pace_sb_deinit(void* obj) {
    if (!obj) return;
    PaceStringBuilder* sb = (PaceStringBuilder*)obj;
    if (sb->buf) {
        free(sb->buf);
        sb->buf = NULL;
    }
}

PaceStringBuilder* pace_sb_new(void) {
    PaceStringBuilder* sb = (PaceStringBuilder*)pace_alloc(sizeof(PaceStringBuilder), pace_sb_deinit);
    sb->capacity = 64;
    sb->length = 0;
    sb->buf = (char*)malloc(sb->capacity);
    if (!sb->buf) pace_panic("Fatal error: out of memory in pace_sb_new");
    sb->buf[0] = '\0';
    return sb;
}

void pace_sb_append(PaceStringBuilder* sb, struct PaceString* s) {
    if (!sb || !s || s->length == 0) return;
    size_t needed = sb->length + s->length + 1;
    if (needed > sb->capacity) {
        size_t new_cap = sb->capacity * 2;
        while (new_cap < needed) new_cap *= 2;
        char* new_buf = (char*)realloc(sb->buf, new_cap);
        if (!new_buf) pace_panic("Fatal error: out of memory in pace_sb_append");
        sb->buf = new_buf;
        sb->capacity = new_cap;
    }
    memcpy(sb->buf + sb->length, s->data, s->length);
    sb->length += s->length;
    sb->buf[sb->length] = '\0';
}

struct PaceString* pace_sb_to_string(PaceStringBuilder* sb) {
    if (!sb) return pace_string_new("");
    return pace_string_from_slice(sb->buf, sb->length);
}

int64_t pace_sb_len(PaceStringBuilder* sb) {
    if (!sb) return 0;
    return (int64_t)sb->length;
}

void pace_sb_clear(PaceStringBuilder* sb) {
    if (!sb) return;
    sb->length = 0;
    if (sb->buf) sb->buf[0] = '\0';
}

