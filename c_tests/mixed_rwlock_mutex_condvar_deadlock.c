// C version of the mixed RwLock + Mutex + Condvar deadlock test
// Compile with: gcc -Iinclude mixed_rwlock_mutex_condvar_deadlock.c -Ltarget/release -ldeloxide -lpthread -o mixed_rwlock_mutex_condvar_deadlock

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
    printf("✔️  Mixed RwLock+Mutex+Condvar deadlock detected!\n");
}

struct shared_resources {
    void* shared_data_rwlock;     // Data that can be read/written
    void* processor_mutex;        // Processing state mutex
    void* data_ready_cv;          // Signals when data is ready
    char processor_state[32];     // "idle" or "processing"
    int shared_data[5];           // Shared data array
};

// Reader thread: reads data, waits for processing completion, then needs processor access
void* reader_thread(void* arg) {
    struct shared_resources* res = (struct shared_resources*)arg;
    

    
    // Reader gets read access to shared data
    RWLOCK_READ(res->shared_data_rwlock);
    printf("Reader: Got read lock on data: [%d, %d, %d, %d, %d]\n", 
           res->shared_data[0], res->shared_data[1], res->shared_data[2], 
           res->shared_data[3], res->shared_data[4]);
    
    // Wait for data processing to be ready
    LOCK_MUTEX(res->processor_mutex);
    while (strcmp(res->processor_state, "idle") == 0) {
        printf("Reader: Waiting for processor to be ready...\n");
        CONDVAR_WAIT(res->data_ready_cv, res->processor_mutex);
    }
    // processor_mutex is reacquired here, but we still hold the RwLock read guard
    
    printf("Reader: Processor is ready, now trying to access it again...\n");
    // This creates the deadlock - we hold RwLock (read) and try to get processor_mutex
    // But the writer thread holds processor_mutex and is trying to get RwLock (write)
    // The reader already has processor_mutex from the wait, but let's simulate 
    // needing it again for a different operation
    UNLOCK_MUTEX(res->processor_mutex); // Release from wait
    
    // Now try to get it again for "final processing"
    printf("Reader: Trying to get processor mutex for final processing...\n");
    LOCK_MUTEX(res->processor_mutex);
    
    printf("Reader: Got final processor access\n");
    // This code is never reached due to deadlock
    
    UNLOCK_MUTEX(res->processor_mutex);
    RWUNLOCK_READ(res->shared_data_rwlock);
    
    return NULL;
}

// Writer thread: manages processing state, signals readiness, then needs data write access
void* writer_thread(void* arg) {
    struct shared_resources* res = (struct shared_resources*)arg;
    

    
    // Small delay to let reader get the read lock first
    usleep(10000); // 10ms
    
    // Writer takes control of processor
    LOCK_MUTEX(res->processor_mutex);
    strcpy(res->processor_state, "processing");
    printf("Writer: Set processor to 'processing' state\n");
    
    // Signal that data processing is ready
    CONDVAR_NOTIFY_ONE(res->data_ready_cv);
    printf("Writer: Notified reader that processing is ready\n");
    
    // Small delay to let reader wake up and try to get processor_mutex again
    usleep(20000); // 20ms
    
    // Now try to write to shared data → DEADLOCK
    // Reader holds RwLock (read) and is trying to get processor_mutex (which we hold)
    // We hold processor_mutex and are trying to get RwLock (write) - blocked by reader
    printf("Writer: Trying to get write access to data...\n");
    RWLOCK_WRITE(res->shared_data_rwlock);
    
    printf("Writer: Got write access to data\n");
    // This code is never reached due to deadlock
    
    // Update data
    for (int i = 0; i < 5; i++) {
        res->shared_data[i] = i * 10;
    }
    
    RWUNLOCK_WRITE(res->shared_data_rwlock);
    UNLOCK_MUTEX(res->processor_mutex);
    
    return NULL;
}

DEFINE_TRACKED_THREAD(reader_thread)
DEFINE_TRACKED_THREAD(writer_thread)

int main() {
    deloxide_init(NULL, deadlock_callback);

    // Create shared resources - simulating a data processing system
    struct shared_resources res;
    res.shared_data_rwlock = deloxide_create_rwlock();
    res.processor_mutex = deloxide_create_mutex();
    res.data_ready_cv = deloxide_create_condvar();
    strcpy(res.processor_state, "idle");
    
    // Initialize shared data
    for (int i = 0; i < 5; i++) {
        res.shared_data[i] = i + 1;
    }

    pthread_t reader, writer;
    CREATE_TRACKED_THREAD(reader, reader_thread, &res);
    CREATE_TRACKED_THREAD(writer, writer_thread, &res);

    // Wait up to 3 seconds for deadlock detection
    for (int i = 0; i < 30 && !deadlock_detected; i++) {
        usleep(100000); // 100ms
    }

    if (deadlock_detected) {
        printf("✅ Mixed RwLock+Mutex+Condvar deadlock test passed\n");
        printf("Deadlock info: %s\n", deadlock_info_json);
        
        // Cleanup
        deloxide_destroy_condvar(res.data_ready_cv);
        deloxide_destroy_mutex(res.processor_mutex);
        deloxide_destroy_rwlock(res.shared_data_rwlock);

        free(deadlock_info_json);
        
        return 0;
    } else {
        fprintf(stderr, "❌ No deadlock detected in mixed primitives test\n");
        
        // Cleanup
        deloxide_destroy_condvar(res.data_ready_cv);
        deloxide_destroy_mutex(res.processor_mutex);
        deloxide_destroy_rwlock(res.shared_data_rwlock);

        
        return 1;
    }
}