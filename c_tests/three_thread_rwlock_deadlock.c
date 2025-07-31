// Compile with: gcc -Iinclude three_thread_rwlock_deadlock.c -Ltarget/release -ldeloxide -lpthread -o rwlock_3thread
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <stdatomic.h>
#include "deloxide.h"

static volatile int deadlock_detected = 0;
static char *deadlock_info_json = NULL;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
    deadlock_info_json = strdup(json_info);
}

struct cycle_args {
    void* locks[3];
    int index;
    atomic_int* ready_count;
};

void* rw_cycle_worker(void* arg) {
    struct cycle_args* a = (struct cycle_args*)arg;
    // Each thread grabs read on i
    RWLOCK_READ(a->locks[a->index]);
    // Signal ready and wait for all threads
    atomic_fetch_add(a->ready_count, 1);
    while (atomic_load(a->ready_count) < 3) {
        sched_yield();
    }
    // Each tries to upgrade to write on (i+1)%3 (held for read by next thread)
    RWLOCK_WRITE(a->locks[(a->index + 1) % 3]);
    // Should never get here
    return NULL;
}

DEFINE_TRACKED_THREAD(rw_cycle_worker)

int main() {
    deloxide_init(NULL, deadlock_callback);

    void* locks[3];
    for (int i = 0; i < 3; ++i) {
        locks[i] = deloxide_create_rwlock();
    }

    atomic_int ready_count = 0;

    pthread_t threads[3];
    struct cycle_args args[3];
    for (int i = 0; i < 3; ++i) {
        args[i].index = i;
        args[i].locks[0] = locks[0];
        args[i].locks[1] = locks[1];
        args[i].locks[2] = locks[2];
        args[i].ready_count = &ready_count;
        CREATE_TRACKED_THREAD(threads[i], rw_cycle_worker, &args[i]);
    }

    // Wait up to 2s for deadlock
    for (int i = 0; i < 20 && !deadlock_detected; ++i)
        usleep(100000);

    if (deadlock_detected) {
        printf("✔ Detected 3-thread RwLock cycle deadlock!\n%s\n", deadlock_info_json);
        return 0;
    } else {
        fprintf(stderr, "No deadlock detected in 3-thread RwLock cycle\n");
        return 1;
    }
}
