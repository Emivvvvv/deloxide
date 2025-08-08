// C version of the producer-consumer condvar deadlock test
// Compile with: gcc -Iinclude condvar_producer_consumer_deadlock.c -Ltarget/release -ldeloxide -lpthread -o condvar_producer_consumer_deadlock

#define _POSIX_C_SOURCE 200112L  // Enable POSIX barriers
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <stdbool.h>
#include "deloxide.h"

#define BUFFER_SIZE 5

static volatile int deadlock_detected = 0;
static char *deadlock_info_json = NULL;

void deadlock_callback(const char* json_info) {
    deadlock_detected = 1;
    deadlock_info_json = strdup(json_info);
    printf("✔️  Producer-Consumer deadlock detected!\n");
}

struct shared_state {
    void* buffer_mutex;
    void* consumer_mutex;
    void* producer_cv;
    int buffer[BUFFER_SIZE];
    int buffer_count;
};

// Producer thread: produces data, waits for buffer space, needs consumer resource
void* producer_thread(void* arg) {
    struct shared_state* state = (struct shared_state*)arg;
    

    
    // Producer holds buffer mutex
    LOCK_MUTEX(state->buffer_mutex);
    printf("Producer: Got buffer mutex\n");
    
    // Initialize buffer to be "full" to force waiting
    for (int i = 0; i < BUFFER_SIZE; i++) {
        state->buffer[i] = i;
    }
    state->buffer_count = BUFFER_SIZE;
    
    // Simulate buffer being full - wait for consumer to make space
    while (state->buffer_count >= BUFFER_SIZE) {
        printf("Producer: Buffer full, waiting for space...\n");
        CONDVAR_WAIT(state->producer_cv, state->buffer_mutex);
    }
    // Buffer mutex is reacquired here
    printf("Producer: Woke up, buffer mutex reacquired\n");
    
    // Try to access consumer resource → DEADLOCK
    // Consumer holds consumer_mutex and is trying to get buffer_mutex
    printf("Producer: Trying to get consumer resource...\n");
    LOCK_MUTEX(state->consumer_mutex);
    
    // This code is never reached
    state->buffer[state->buffer_count++] = 42;
    printf("Producer: Added item to buffer\n");
    
    UNLOCK_MUTEX(state->consumer_mutex);
    UNLOCK_MUTEX(state->buffer_mutex);
    
    return NULL;
}

// Consumer thread: holds consumer resource, signals producer, needs buffer
void* consumer_thread(void* arg) {
    struct shared_state* state = (struct shared_state*)arg;
    

    
    // Small delay to let producer start waiting
    usleep(50000); // 50ms
    
    // Consumer holds its resource first
    LOCK_MUTEX(state->consumer_mutex);
    printf("Consumer: Got consumer mutex\n");
    
    // Actually make space in the buffer so producer can proceed
    {
        LOCK_MUTEX(state->buffer_mutex);
        if (state->buffer_count > 0) {
            state->buffer_count--;
            printf("Consumer: Removed item from buffer, space available\n");
        }
        UNLOCK_MUTEX(state->buffer_mutex);
    }
    
    // Signal producer that space is available
    printf("Consumer: Signaling producer...\n");
    CONDVAR_NOTIFY_ONE(state->producer_cv);
    
    // Small delay to let producer wake up and try to get consumer_mutex
    usleep(50000); // 50ms
    
    // Try to access buffer → DEADLOCK  
    // Producer holds buffer_mutex and is trying to get consumer_mutex (which we hold)
    printf("Consumer: Trying to get buffer mutex...\n");
    LOCK_MUTEX(state->buffer_mutex);
    
    // This code is never reached
    printf("Consumer: Got buffer mutex\n");
    
    UNLOCK_MUTEX(state->buffer_mutex);
    UNLOCK_MUTEX(state->consumer_mutex);
    
    return NULL;
}

DEFINE_TRACKED_THREAD(producer_thread)
DEFINE_TRACKED_THREAD(consumer_thread)

int main() {
    deloxide_init(NULL, deadlock_callback);

    // Create shared state
    struct shared_state state;
    state.buffer_mutex = deloxide_create_mutex();
    state.consumer_mutex = deloxide_create_mutex();
    state.producer_cv = deloxide_create_condvar();
    state.buffer_count = 0;

    pthread_t producer, consumer;
    CREATE_TRACKED_THREAD(producer, producer_thread, &state);
    CREATE_TRACKED_THREAD(consumer, consumer_thread, &state);

    // Wait up to 3 seconds for deadlock detection
    for (int i = 0; i < 30 && !deadlock_detected; i++) {
        usleep(100000); // 100ms
    }

    if (deadlock_detected) {
        printf("✅ Producer-Consumer condvar deadlock test passed\n");
        printf("Deadlock info: %s\n", deadlock_info_json);
        
        // Cleanup
        deloxide_destroy_condvar(state.producer_cv);
        deloxide_destroy_mutex(state.buffer_mutex);
        deloxide_destroy_mutex(state.consumer_mutex);

        free(deadlock_info_json);
        
        return 0;
    } else {
        fprintf(stderr, "❌ No deadlock detected in producer-consumer test\n");
        
        // Cleanup
        deloxide_destroy_condvar(state.producer_cv);
        deloxide_destroy_mutex(state.buffer_mutex);
        deloxide_destroy_mutex(state.consumer_mutex);

        
        return 1;
    }
}