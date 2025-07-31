// Compile with: gcc -Iinclude rwlock_upgrade_deadlock.c -Ltarget/release -ldeloxide -lpthread -o rwlock_upgrade_deadlock
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

struct upgrade_args {
    void* rwlock;
    atomic_int* ready_count;
};

void* upgrade_worker(void* arg) {
    struct upgrade_args* a = (struct upgrade_args*)arg;
    RWLOCK_READ(a->rwlock);
    // Signal ready and wait for all threads
    atomic_fetch_add(a->ready_count, 1);
    while (atomic_load(a->ready_count) < 2) {
        sched_yield();
    }
    // Both try to upgrade at once (classic upgrade deadlock)
    RWLOCK_WRITE(a->rwlock);
    return NULL;
}

DEFINE_TRACKED_THREAD(upgrade_worker)

int main() {
    deloxide_init(NULL, deadlock_callback);

    void* rwlock = deloxide_create_rwlock();

    atomic_int ready_count = 0;

    pthread_t threads[2];
    struct upgrade_args args[2];
    for (int i = 0; i < 2; ++i) {
        args[i].rwlock = rwlock;
        args[i].ready_count = &ready_count;
        CREATE_TRACKED_THREAD(threads[i], upgrade_worker, &args[i]);
    }

    // Wait up to 2s for deadlock
    for (int i = 0; i < 20 && !deadlock_detected; ++i)
        usleep(100000);

    if (deadlock_detected) {
        printf("✔ Detected RwLock upgrade deadlock!\n%s\n", deadlock_info_json);
        return 0;
    } else {
        fprintf(stderr, "No deadlock detected in upgrade deadlock test\n");
        return 1;
    }
}
