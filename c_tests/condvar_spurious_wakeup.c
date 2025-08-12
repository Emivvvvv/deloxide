// Compile with: gcc -Iinclude c_tests/condvar_spurious_wakeup.c -Ltarget/release -ldeloxide -lpthread -o bin/condvar_spurious_wakeup

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdatomic.h>
#include "deloxide.h"
#include "test_util.h"

struct shared_state {
    void* mutex;
    void* cv;
    volatile bool predicate;
    atomic_int woke_count;
};

void* waiter(void* arg) {
    struct shared_state* s = (struct shared_state*)arg;
    LOCK_MUTEX(s->mutex);
    while (!s->predicate) {
        CONDVAR_WAIT(s->cv, s->mutex);
    }
    atomic_fetch_add(&s->woke_count, 1);
    UNLOCK_MUTEX(s->mutex);
    return NULL;
}

DEFINE_TRACKED_THREAD(waiter)

int main() {
    deloxide_test_init();

    struct shared_state s;
    s.mutex = deloxide_create_mutex();
    s.cv = deloxide_create_condvar();
    s.predicate = false;
    atomic_init(&s.woke_count, 0);

    pthread_t t;
    CREATE_TRACKED_THREAD(t, waiter, &s);

    // Fire a few notifications before predicate is true (may spuriously wake)
    for (int i = 0; i < 3; ++i) {
        CONDVAR_NOTIFY_ONE(s.cv);
        usleep(5000);
    }

    // Set predicate and notify to complete
    LOCK_MUTEX(s.mutex);
    s.predicate = true;
    UNLOCK_MUTEX(s.mutex);
    CONDVAR_NOTIFY_ONE(s.cv);

    // Give the thread time to finish and ensure no deadlock was detected
    usleep(200000); // 200ms

    if (DEADLOCK_FLAG) {
        fprintf(stderr, "❌ False deadlock detected in spurious wakeup test\n");
        return 1;
    }

    printf("✔ No deadlock detected with spurious condvar wakeups (expected)\n");
    return 0;
}


