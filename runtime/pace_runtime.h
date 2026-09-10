#ifndef PACE_RUNTIME_H
#define PACE_RUNTIME_H

#ifdef __cplusplus
extern "C" {
#endif

void PACE_RETAIN(long long obj);
void PACE_RELEASE(long long obj);

void pace_print_int(long long val);
void pace_print_string(const char* val);

#ifdef __cplusplus
}
#endif

#endif // PACE_RUNTIME_H
