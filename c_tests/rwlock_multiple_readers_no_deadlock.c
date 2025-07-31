// Compile with: gcc -Iinclude rwlock_multiple_readers_no_deadlock.c -Ltarget/release -ldeloxide -lpthread -o rwlock_readers_no_deadlock
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include "deloxide.h"

static volatile int deadlock_detected = 0;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
}

struct reader_args {
    void* rwlock;
};

void* reader(void* arg) {
    struct reader_args* a = (struct reader_args*)arg;
    RWLOCK_READ(a->rwlock);
    usleep(50000); // 50 ms
    RWUNLOCK_READ(a->rwlock);
    return NULL;
}

DEFINE_TRACKED_THREAD(reader)

int main() {
    deloxide_init(NULL, deadlock_callback);

    void* rwlock = deloxide_create_rwlock();

    pthread_t threads[4];
    struct reader_args args[4];
    for (int i = 0; i < 4; ++i) {
        args[i].rwlock = rwlock;
        CREATE_TRACKED_THREAD(threads[i], reader, &args[i]);
    }

    for (int i = 0; i < 4; ++i) {
        pthread_join(threads[i], NULL);
    }

    // There should be no deadlock notification
    if (deadlock_detected) {
        fprintf(stderr, "False deadlock detected with multiple readers!\n");
        return 1;
    } else {
        printf("✔ No deadlock detected with multiple readers (expected)\n");
        return 0;
    }
}
