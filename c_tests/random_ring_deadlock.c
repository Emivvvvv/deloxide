// Compile with: gcc -Iinclude random_ring_deadlock.c -Ltarget/release -ldeloxide -lpthread -o random_ring

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <pthread.h>
#include "deloxide.h"
#include "test_util.h"

// Use shared test util globals/callback

struct ring_args {
    int index, n;
    void** locks;
};

void* ring_worker(void* arg) {
    struct ring_args* a = arg;
    unsigned int seed = (unsigned int)(time(NULL) ^ deloxide_get_thread_id());
    int i = a->index;
    void* first  = a->locks[i];
    void* second = a->locks[(i + 1) % a->n];

    usleep((rand_r(&seed) % 50) * 1000);
    LOCK_MUTEX(first);
    usleep(((rand_r(&seed) % 50) + 50) * 1000);
    LOCK_MUTEX(second);

    return NULL;
}

DEFINE_TRACKED_THREAD(ring_worker)

int main() {
    srand((unsigned)time(NULL));
    deloxide_test_init();

    int n = (rand() % 6) + 3;  // 3..8
    printf("→ testing a ring of %d threads\n", n);

    void* locks[n];
    for (int i = 0; i < n; i++) {
        locks[i] = deloxide_create_mutex();
    }

    pthread_t threads[n];
    struct ring_args args[n];
    for (int i = 0; i < n; i++) {
        args[i] = (struct ring_args){ .index = i, .n = n, .locks = locks };
        CREATE_TRACKED_THREAD(threads[i], ring_worker, &args[i]);
    }

    // Wait up to 5 s
    wait_for_deadlock_ms(5000, 100);

    if (DEADLOCK_FLAG) {
        printf("Deadlock detected (ring of %d)! Info:\n%s\n", n, DEADLOCK_INFO);
        return 0;
    } else {
        fprintf(stderr, "No deadlock in ring test\n");
        return 1;
    }
}
