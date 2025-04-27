// Compile with: gcc -Iinclude two_thread_deadlock.c -Ltarget/release -ldeloxide -lpthread -o two_thread

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include "deloxide.h"

static volatile int deadlock_detected = 0;
static char *deadlock_info_json = NULL;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
    deadlock_info_json = strdup(json_info);
}

struct two_args {
    void* lock_a;
    void* lock_b;
};

void* cross_lock(void* arg) {
    struct two_args* a = arg;
    LOCK(a->lock_a);
    usleep(100000);  // 100 ms
    LOCK(a->lock_b);
    return NULL;
}

DEFINE_TRACKED_THREAD(cross_lock)

int main() {
    deloxide_init(NULL, deadlock_callback);

    void* ra = deloxide_create_mutex();
    void* rb = deloxide_create_mutex();

    struct two_args arg1 = { .lock_a = ra, .lock_b = rb };
    struct two_args arg2 = { .lock_a = rb, .lock_b = ra };

    pthread_t t1, t2;
    CREATE_TRACKED_THREAD(t1, cross_lock, &arg1);
    CREATE_TRACKED_THREAD(t2, cross_lock, &arg2);


    // Wait up to 2 s
    for (int i = 0; i < 20 && !deadlock_detected; i++) {
        usleep(100000);
    }

    if (deadlock_detected) {
        printf("Deadlock detected (2-thread cross)!\n%s\n", deadlock_info_json);
        return 0;
    } else {
        fprintf(stderr, "No deadlock detected in 2-thread test\n");
        return 1;
    }
}
