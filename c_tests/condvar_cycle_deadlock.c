// C version of the basic condvar deadlock test
// Compile with: gcc -Iinclude condvar_cycle_deadlock.c -Ltarget/release -ldeloxide -lpthread -o condvar_cycle_deadlock

#define _POSIX_C_SOURCE 200112L  // Enable POSIX barriers
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <stdbool.h>
#include "deloxide.h"

static volatile int deadlock_detected = 0;
static char *deadlock_info_json = NULL;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
    deadlock_info_json = strdup(json_info);
    printf("✔️  Condvar cycle detected!\n");
}

struct thread_args {
    void* mutex_a;
    void* mutex_b;
    void* condvar;
    volatile bool* ready;
};

// Thread 1: waits on condvar, then needs mutex B
void* thread1_wait_then_lock(void* arg) {
    struct thread_args* args = (struct thread_args*)arg;
    
    LOCK_MUTEX(args->mutex_a);
    printf("Thread1: Got mutex A\n");
    
    // Wait on condvar until ready becomes true
    while (!*(args->ready)) {
        printf("Thread1: Waiting on condvar...\n");
        CONDVAR_WAIT(args->condvar, args->mutex_a);
    }
    printf("Thread1: Woke up from condvar, mutex A reacquired\n");
    
    // Try to get mutex B → DEADLOCK (should block forever)
    printf("Thread1: Trying to get mutex B...\n");
    LOCK_MUTEX(args->mutex_b);
    
    // This should never be reached due to deadlock
    printf("Thread1: ERROR - Got mutex B when should be deadlocked!\n");
    UNLOCK_MUTEX(args->mutex_b);
    UNLOCK_MUTEX(args->mutex_a);
    
    return NULL;
}

// Thread 2: holds mutex B, signals condvar, then needs mutex A
void* thread2_signal_then_lock(void* arg) {
    struct thread_args* args = (struct thread_args*)arg;
    
    // Small delay to ensure thread1 gets to wait first
    usleep(10000); // 10ms
    
    LOCK_MUTEX(args->mutex_b);
    printf("Thread2: Got mutex B\n");
    
    // Set ready and signal condvar
    {
        LOCK_MUTEX(args->mutex_a);
        *(args->ready) = true;
        printf("Thread2: Set ready=true, signaling condvar...\n");
        CONDVAR_NOTIFY_ONE(args->condvar);
        UNLOCK_MUTEX(args->mutex_a);
    }
    
    // Small delay to let thread1 wake up and try to get mutex B
    usleep(20000); // 20ms
    
    // Try to get mutex A → DEADLOCK (should block forever)
    printf("Thread2: Trying to get mutex A...\n");
    LOCK_MUTEX(args->mutex_a);
    
    // This should never be reached due to deadlock
    printf("Thread2: ERROR - Got mutex A when should be deadlocked!\n");
    UNLOCK_MUTEX(args->mutex_a);
    UNLOCK_MUTEX(args->mutex_b);
    
    return NULL;
}

DEFINE_TRACKED_THREAD(thread1_wait_then_lock)
DEFINE_TRACKED_THREAD(thread2_signal_then_lock)

int main() {
    deloxide_init(NULL, deadlock_callback);

    // Create shared resources
    void* mutex_a = deloxide_create_mutex();
    void* mutex_b = deloxide_create_mutex();
    void* condvar = deloxide_create_condvar();
    volatile bool ready = false;
    
    struct thread_args args = {
        .mutex_a = mutex_a,
        .mutex_b = mutex_b,
        .condvar = condvar,
        .ready = &ready
    };

    pthread_t t1, t2;
    CREATE_TRACKED_THREAD(t1, thread1_wait_then_lock, &args);
    CREATE_TRACKED_THREAD(t2, thread2_signal_then_lock, &args);

    // Wait up to 3 seconds for deadlock detection
    for (int i = 0; i < 30 && !deadlock_detected; i++) {
        usleep(100000); // 100ms
    }

    if (deadlock_detected) {
        printf("✅ Condvar cycle deadlock test passed\n");
        printf("Deadlock info: %s\n", deadlock_info_json);
        
        // Cleanup
        deloxide_destroy_condvar(condvar);
        deloxide_destroy_mutex(mutex_a);
        deloxide_destroy_mutex(mutex_b);

        free(deadlock_info_json);
        
        return 0;
    } else {
        fprintf(stderr, "❌ No deadlock detected in condvar cycle test\n");
        
        // Cleanup
        deloxide_destroy_condvar(condvar);
        deloxide_destroy_mutex(mutex_a);
        deloxide_destroy_mutex(mutex_b);

        
        return 1;
    }
}