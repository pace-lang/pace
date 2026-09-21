#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "pace_runtime.h"

typedef struct PaceClosure {
    void* func;
    void* env;
} PaceClosure;

struct __Env___closure_1 {
};

struct __Env___closure_2 {
};

struct __Env___closure_3 {
    long long _1;
};



void closures_testClosures(void);
long long __closure_1(void* __env, long long _0, long long _1);
long long __closure_2(void* __env, long long _0, long long _1);
long long __closure_3(void* __env, long long _0);
void pace_main(void);

void closures_testClosures(void) {
    struct PaceString* _0 = NULL;
    long long _1 = 0;
    PaceClosure* _2 = NULL;
    PaceClosure* _3 = NULL;
    long long _4 = 0;
    long long _5 = 0;
    PaceClosure* _6 = NULL;
    long long _7 = 0;
    long long _8 = 0;
    long long _9 = 0;
    long long _10 = 0;
    PaceClosure* _11 = NULL;
    PaceClosure* _12 = NULL;
    long long _13 = 0;
    long long _14 = 0;
    PaceClosure* _15 = NULL;
    long long _16 = 0;
    long long _17 = 0;
    long long _18 = 0;
    long long _19 = 0;
    long long _20 = 0;
    long long _21 = 0;
    PaceClosure* _22 = NULL;
    PaceClosure* _23 = NULL;
    long long _24 = 0;
    PaceClosure* _25 = NULL;
    long long _26 = 0;
    long long _27 = 0;
    long long _28 = 0;
    long long _29 = 0;
    long long _30 = 0;

closures_testClosures_bb_0:
    _0 = pace_string_new("--- Closures & Lambdas ---");
    pace_print_string((void*)_0);
    _2 = (PaceClosure*)memcpy(pace_alloc(sizeof(PaceClosure), NULL), &(PaceClosure){ .func = (void*)__closure_1, .env = NULL }, sizeof(PaceClosure));
    _3 = _2;
    _4 = 10;
    _5 = 20;
    _6 = _3;
    _7 = ((long long (*)(void*, long long, long long))_6->func)(_6->env, _4, _5);
    _8 = _7;
    _9 = _8;
    pace_print_int(_9);
    _11 = (PaceClosure*)memcpy(pace_alloc(sizeof(PaceClosure), NULL), &(PaceClosure){ .func = (void*)__closure_2, .env = NULL }, sizeof(PaceClosure));
    _12 = _11;
    _13 = 5;
    _14 = 5;
    _15 = _12;
    _16 = ((long long (*)(void*, long long, long long))_15->func)(_15->env, _13, _14);
    _17 = _16;
    _18 = _17;
    pace_print_int(_18);
    _20 = 100;
    _21 = _20;
    _22 = (PaceClosure*)memcpy(pace_alloc(sizeof(PaceClosure), NULL), &(PaceClosure){ .func = (void*)__closure_3, .env = memcpy(pace_alloc(sizeof(struct __Env___closure_3), NULL), &(struct __Env___closure_3){ _21 }, sizeof(struct __Env___closure_3)) }, sizeof(PaceClosure));
    _23 = _22;
    _24 = 50;
    _25 = _23;
    _26 = ((long long (*)(void*, long long))_25->func)(_25->env, _24);
    _27 = _26;
    _28 = _27;
    pace_print_int(_28);
    _30 = 0;
    return;
}

long long __closure_1(void* __env, long long _0, long long _1) {
    long long _2 = 0;
    long long _3 = 0;
    long long _4 = 0;
    long long _5 = 0;

    struct __Env___closure_1* __env_ptr = (struct __Env___closure_1*)__env;

__closure_1_bb_0:
    _2 = _0;
    _3 = _1;
    _4 = _2 + _3;
    _5 = _4;
    return _5;
}

long long __closure_2(void* __env, long long _0, long long _1) {
    long long _2 = 0;
    long long _3 = 0;
    long long _4 = 0;
    long long _5 = 0;

    struct __Env___closure_2* __env_ptr = (struct __Env___closure_2*)__env;

__closure_2_bb_0:
    _2 = _0;
    _3 = _1;
    _4 = _2 * _3;
    _5 = _4;
    return _5;
}

long long __closure_3(void* __env, long long _0) {
    long long _1 = 0;
    long long _2 = 0;
    long long _3 = 0;
    long long _4 = 0;
    long long _5 = 0;

    struct __Env___closure_3* __env_ptr = (struct __Env___closure_3*)__env;
    _1 = __env_ptr->_1;

__closure_3_bb_0:
    _2 = _0;
    _3 = _1;
    _4 = _2 + _3;
    _5 = _4;
    return _5;
}

void pace_main(void) {
    long long _1 = 0;

main_bb_0:
    closures_testClosures();
    _1 = 0;
    return;
}

void pace_init() {
    long long _0 = 0;

init_bb_0:
    _0 = 0;
}

int main() {
    pace_init();
    pace_main();
    return 0;
}
