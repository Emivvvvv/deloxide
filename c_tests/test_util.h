#ifndef DELOXIDE_TEST_UTIL_H
#define DELOXIDE_TEST_UTIL_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "../include/deloxide.h"

#define DEADLOCK_TIMEOUT_MS 3000
#define NO_DEADLOCK_TIMEOUT_MS 500

static volatile int g_deadlock_detected = 0;
static char *g_deadlock_info_json = NULL;

static inline void deloxide_test_callback(const char* json_info) {
    g_deadlock_detected = 1;
    if (json_info) {
        g_deadlock_info_json = strdup(json_info);
    }
}

static inline void deloxide_test_init(void) {
    deloxide_init(NULL, deloxide_test_callback);
}

static inline int wait_for_deadlock_ms(int total_ms, int step_ms) {
    int steps = total_ms / step_ms;
    for (int i = 0; i < steps && !g_deadlock_detected; ++i) {
        usleep(step_ms * 1000);
    }
    return g_deadlock_detected;
}

#define DEADLOCK_FLAG g_deadlock_detected
#define DEADLOCK_INFO g_deadlock_info_json

#endif // DELOXIDE_TEST_UTIL_H


