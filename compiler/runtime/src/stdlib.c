#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif
#include <errno.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#ifdef _WIN32
#include <direct.h>
#include <windows.h>
#ifndef S_ISREG
#define S_ISREG(m) (((m) & _S_IFMT) == _S_IFREG)
#endif
#ifndef S_ISDIR
#define S_ISDIR(m) (((m) & _S_IFMT) == _S_IFDIR)
#endif
#else
#include <unistd.h>
#endif
#include "../pace_runtime.h"

static int g_pace_argc = 0;
static char** g_pace_argv = NULL;

void pace_set_args(int argc, char** argv) {
    g_pace_argc = argc;
    g_pace_argv = argv;
}

PaceList* pace_process_args(void) {
    PaceList* out = pace_list_new(g_pace_argc > 4 ? (size_t)g_pace_argc : 4, pace_release);
    for (int i = 0; i < g_pace_argc; i++) {
        struct PaceString* s = pace_string_new(g_pace_argv[i] ? g_pace_argv[i] : "");
        pace_list_push(out, s);
        pace_release(s);
    }
    return out;
}

struct PaceString* pace_process_cwd(void) {
    char buf[4096];
#ifdef _WIN32
    if (_getcwd(buf, (int)sizeof(buf)) != NULL) {
#else
    if (getcwd(buf, sizeof(buf)) != NULL) {
#endif
        return pace_string_new(buf);
    }
    return pace_string_new("");
}

void pace_process_exit(int64_t code) {
    pace_thread_pool_shutdown();
    exit((int)code);
}

void pace_eprintln(struct PaceString* val) {
    if (val && val->data) {
        fprintf(stderr, "%s\n", val->data);
    } else {
        fprintf(stderr, "\n");
    }
}

PaceResult pace_read_line(void) {
    size_t cap = 128;
    size_t len = 0;
    char* line = (char*)malloc(cap);
    if (!line) {
        pace_panic("Fatal error: out of memory in pace_read_line");
    }
    int c;
    while ((c = fgetc(stdin)) != EOF) {
        if (c == '\n') break;
        if (len + 1 >= cap) {
            cap *= 2;
            char* next = (char*)realloc(line, cap);
            if (!next) {
                free(line);
                pace_panic("Fatal error: out of memory in pace_read_line");
            }
            line = next;
        }
        line[len++] = (char)c;
    }
    if (c == EOF && len == 0) {
        free(line);
        struct PaceString* err = pace_string_new("EOF");
        return (PaceResult){0, NULL, err};
    }
    while (len > 0 && line[len - 1] == '\r') {
        len--;
    }
    line[len] = '\0';
    struct PaceString* res = pace_string_new(line);
    free(line);
    return (PaceResult){1, res, NULL};
}

int pace_file_exists(struct PaceString* path) {
    if (!path || !path->data) return 0;
    struct stat st;
    if (stat(path->data, &st) != 0) return 0;
    return S_ISREG(st.st_mode) ? 1 : 0;
}

PaceResult pace_file_read_text(struct PaceString* path) {
    if (!path || !path->data) {
        return (PaceResult){0, NULL, pace_string_new("Invalid path")};
    }
    FILE* f = fopen(path->data, "rb");
    if (!f) {
        return (PaceResult){0, NULL, pace_string_new(strerror(errno))};
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (sz < 0) sz = 0;
    char* buf = (char*)malloc((size_t)sz + 1);
    if (!buf) {
        fclose(f);
        pace_panic("Fatal error: out of memory in pace_file_read_text");
    }
    size_t read_n = fread(buf, 1, (size_t)sz, f);
    buf[read_n] = '\0';
    fclose(f);
    struct PaceString* res = pace_string_new(buf);
    free(buf);
    return (PaceResult){1, res, NULL};
}

PaceResult pace_file_write_text(struct PaceString* path, struct PaceString* contents) {
    if (!path || !path->data) {
        return (PaceResult){0, NULL, pace_string_new("Invalid path")};
    }
    FILE* f = fopen(path->data, "wb");
    if (!f) {
        return (PaceResult){0, NULL, pace_string_new(strerror(errno))};
    }
    if (contents && contents->data && contents->length > 0) {
        size_t written = fwrite(contents->data, 1, contents->length, f);
        if (written != contents->length) {
            int err = errno;
            fclose(f);
            return (PaceResult){0, NULL, pace_string_new(strerror(err))};
        }
    }
    fclose(f);
    return (PaceResult){1, NULL, NULL};
}

int pace_dir_exists(struct PaceString* path) {
    if (!path || !path->data) return 0;
    struct stat st;
    if (stat(path->data, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

PaceResult pace_dir_make(struct PaceString* path) {
    if (!path || !path->data) {
        return (PaceResult){0, NULL, pace_string_new("Invalid path")};
    }
#ifdef _WIN32
    int rc = _mkdir(path->data);
#else
    int rc = mkdir(path->data, 0755);
#endif
    if (rc != 0 && errno != EEXIST) {
        return (PaceResult){0, NULL, pace_string_new(strerror(errno))};
    }
    return (PaceResult){1, NULL, NULL};
}

struct PaceString* pace_env_get(struct PaceString* key) {
    if (!key || !key->data) return NULL;
    const char* v = getenv(key->data);
    if (!v) return NULL;
    return pace_string_new(v);
}

void pace_env_set(struct PaceString* key, struct PaceString* val) {
    if (!key || !key->data) return;
    const char* v = (val && val->data) ? val->data : "";
#ifdef _WIN32
    _putenv_s(key->data, v);
#else
    setenv(key->data, v, 1);
#endif
}

int pace_env_has(struct PaceString* key) {
    if (!key || !key->data) return 0;
    return getenv(key->data) != NULL ? 1 : 0;
}

struct PaceString* pace_os_name(void) {
#if defined(__APPLE__)
    return pace_string_new("macos");
#elif defined(_WIN32)
    return pace_string_new("windows");
#else
    return pace_string_new("linux");
#endif
}

int64_t pace_time_now_ms(void) {
    struct timespec ts;
#ifdef _WIN32
    timespec_get(&ts, TIME_UTC);
#else
    clock_gettime(CLOCK_REALTIME, &ts);
#endif
    return (int64_t)ts.tv_sec * 1000 + (int64_t)(ts.tv_nsec / 1000000);
}

void pace_time_sleep_ms(int64_t ms) {
    if (ms <= 0) return;
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
#endif
}

double pace_math_sqrt(double x) { return sqrt(x); }
double pace_math_pow(double x, double y) { return pow(x, y); }
double pace_math_abs(double x) { return fabs(x); }
double pace_math_round(double x) { return round(x); }
double pace_math_floor(double x) { return floor(x); }
double pace_math_ceil(double x) { return ceil(x); }
double pace_math_sin(double x) { return sin(x); }
double pace_math_cos(double x) { return cos(x); }
double pace_math_tan(double x) { return tan(x); }

