// Compile with: gcc -Iinclude c_tests/mixed_three_thread_deadlock.c -Ltarget/release -ldeloxide -lpthread -o bin/mixed_three_thread_deadlock

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
    void* m1;
    void* m2;
    void* rw;
    void* cv;
    atomic_int ready;
};

// Thread A: holds m2, waits on cv, then attempts rw.write() (A -> B)
void* thread_a(void* arg) {
    struct shared_state* s = (struct shared_state*)arg;
    LOCK_MUTEX(s->m2);
    while (!atomic_load(&s->ready)) {
        CONDVAR_WAIT(s->cv, s->m2);
    }
    // m2 reacquired here; now attempt write on rw → blocks due to reader
    RWLOCK_WRITE(s->rw);
    // Never reached
    return NULL;
}

// Thread B: holds rw.read(), then attempts to lock m1 (B -> C)
void* thread_b(void* arg) {
    struct shared_state* s = (struct shared_state*)arg;
    RWLOCK_READ(s->rw);
    usleep(30000); // 30ms
    LOCK_MUTEX(s->m1);
    return NULL;
}

// Thread C: holds m1, sets ready, notifies cv, then attempts m2 (C -> A)
void* thread_c(void* arg) {
    struct shared_state* s = (struct shared_state*)arg;
    LOCK_MUTEX(s->m1);
    usleep(20000); // let A start waiting and B acquire read lock
    atomic_store(&s->ready, 1);
    CONDVAR_NOTIFY_ONE(s->cv);
    usleep(20000);
    LOCK_MUTEX(s->m2);
    return NULL;
}

DEFINE_TRACKED_THREAD(thread_a)
DEFINE_TRACKED_THREAD(thread_b)
DEFINE_TRACKED_THREAD(thread_c)

int main() {
    deloxide_test_init();

    struct shared_state s;
    s.m1 = deloxide_create_mutex();
    s.m2 = deloxide_create_mutex();
    s.rw = deloxide_create_rwlock();
    s.cv = deloxide_create_condvar();
    atomic_init(&s.ready, 0);

    pthread_t a, b, c;
    CREATE_TRACKED_THREAD(a, thread_a, &s);
    CREATE_TRACKED_THREAD(b, thread_b, &s);
    CREATE_TRACKED_THREAD(c, thread_c, &s);

    // Wait up to 3 seconds for deadlock detection
    wait_for_deadlock_ms(3000, 100);

    if (DEADLOCK_FLAG) {
        printf("✅ Mixed three-thread Mutex/RwLock/Condvar deadlock detected\n%s\n", DEADLOCK_INFO);
        return 0;
    } else {
        fprintf(stderr, "❌ No deadlock detected in mixed three-thread test\n");
        return 1;
    }
}


