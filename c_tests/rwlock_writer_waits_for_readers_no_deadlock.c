// Compile with: gcc -Iinclude rwlock_writer_waits_for_readers_no_deadlock.c -Ltarget/release -ldeloxide -lpthread -o rwlock_writer_waits_no_deadlock
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include "deloxide.h"
#include "test_util.h"

// Use shared test util globals/callback

struct rwlock_arg {
    void* rwlock;
};

void* reader(void* arg) {
    struct rwlock_arg* a = (struct rwlock_arg*)arg;
    RWLOCK_READ(a->rwlock);
    usleep(100000); // 100 ms
    RWUNLOCK_READ(a->rwlock);
    return NULL;
}

void* writer(void* arg) {
    struct rwlock_arg* a = (struct rwlock_arg*)arg;
    RWLOCK_WRITE(a->rwlock);
    RWUNLOCK_WRITE(a->rwlock);
    return NULL;
}

DEFINE_TRACKED_THREAD(reader)
DEFINE_TRACKED_THREAD(writer)

int main() {
    deloxide_test_init();

    void* rwlock = deloxide_create_rwlock();

    pthread_t t_reader, t_writer;
    struct rwlock_arg arg = { .rwlock = rwlock };

    // Reader holds the lock first
    CREATE_TRACKED_THREAD(t_reader, reader, &arg);
    usleep(10000); // 10 ms to let reader get the lock

    // Writer will wait until reader is done (but not deadlock!)
    CREATE_TRACKED_THREAD(t_writer, writer, &arg);

    pthread_join(t_reader, NULL);
    pthread_join(t_writer, NULL);

    if (DEADLOCK_FLAG) {
        fprintf(stderr, "False deadlock detected with writer waiting for readers!\n");
        return 1;
    } else {
        printf("✔ No deadlock detected with writer waiting for readers (expected)\n");
        return 0;
    }
}
