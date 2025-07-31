// Compile with: gcc -Iinclude dining_philosophers_deadlock.c -Ltarget/release -ldeloxide -lpthread -o dining_philosophers

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include "deloxide.h"

#define N 5

static volatile int deadlock_detected = 0;
static char *deadlock_info_json = NULL;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
    deadlock_info_json = strdup(json_info);
}

struct phil_args {
    void* forks[N];
    int index;
};

void* philosopher(void* arg) {
    struct phil_args* a = arg;
    void* left  = a->forks[a->index];
    void* right = a->forks[(a->index + 1) % N];

    LOCK_MUTEX(left);
    usleep(100000);  // 100 ms
    LOCK_MUTEX(right);

    usleep(500000);  // eating
    return NULL;
}

DEFINE_TRACKED_THREAD(philosopher)

int main() {
    deloxide_init(NULL, deadlock_callback);

    // Create forks
    void* forks[N];
    for(int i = 0; i < N; i++) {
        forks[i] = deloxide_create_mutex();
    }

    // Launch philosophers
    pthread_t threads[N];
    struct phil_args args[N];
    for (int i = 0; i < N; i++) {
        args[i].index = i;
        memcpy(args[i].forks, forks, sizeof(forks));
        CREATE_TRACKED_THREAD(threads[i], philosopher, &args[i]);
    }

    // Wait up to 3 s for deadlock
    for (int i = 0; i < 30 && !deadlock_detected; i++) {
        usleep(100000);
    }

    if (deadlock_detected) {
        printf("Deadlock detected (Dining Philosophers)! Info:\n%s\n", deadlock_info_json);
        return 0;
    } else {
        fprintf(stderr, "No deadlock detected in Dining Philosophers test\n");
        return 1;
    }
}
