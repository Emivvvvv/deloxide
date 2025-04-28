/*
 * @file deloxide.h
 * @brief C API for the Rust deloxide deadlock detection library.
 *
 * Deloxide is a cross-language deadlock detector that helps identify potential
 * deadlocks in multi-threaded applications before they cause your program to hang.
 * It works by tracking mutex operations and relationships between threads and locks,
 * providing real-time detection of deadlock cycles.
 *
 * This header provides the C API for integrating Deloxide into C applications.
 *
 * Quick start:
 * 1. Call deloxide_init() to initialize the library
 * 2. Use deloxide_create_mutex() to create tracked mutexes
 * 3. Use the LOCK() and UNLOCK() macros to operate on mutexes
 * 4. Create threads with CREATE_TRACKED_THREAD() macro for proper tracking
 * 5. If a deadlock is detected, your callback function will be invoked
 *
 * Example:
 * ```c
 * void deadlock_callback(const char* json_info) {
 *     printf("Deadlock detected! Info: %s\n", json_info);
 * }
 *
 * void* worker(void* arg) {
 *     void* mutex = (void*)arg;
 *     LOCK(mutex);
 *     // Critical section
 *     UNLOCK(mutex);
 *     return NULL;
 * }
 *
 * DEFINE_TRACKED_THREAD(worker)
 *
 * int main() {
 *     deloxide_init(NULL, deadlock_callback);
 *     void* mutex = deloxide_create_mutex();
 *
 *     pthread_t thread;
 *     CREATE_TRACKED_THREAD(thread, worker, mutex);
 *
 *     pthread_join(thread, NULL);
 *     deloxide_destroy_mutex(mutex);
 *     return 0;
 * }
 * ```
 */

#ifndef DELOXIDE_H
#define DELOXIDE_H

#include <stddef.h>

/*
 * --- High-Level Macros for Easier Tracked Usage ---
 *
 * These macros simplify correct usage of Deloxide from C.
 *
 * 1. Tracked Threads:
 *    - Automatically register spawn and exit events for threads.
 *    - Example usage:
 *
 *        void* worker(void* unused) {
 *            LOCK(mutex);
 *            // do work
 *            UNLOCK(mutex);
 *            return NULL;
 *        }
 *
 *        DEFINE_TRACKED_THREAD(worker)
 *
 *        pthread_t t;
 *        CREATE_TRACKED_THREAD(t, worker);
 *
 * 2. Tracked Mutexes:
 *    - Simplify locking and unlocking tracked mutexes.
 *    - Always uses correct thread ID internally.
 *    - Example:
 *
 *        LOCK(mutex);
 *        ... critical section ...
 *        UNLOCK(mutex);
 */

#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

/**
 * @brief Define a tracked thread function
 *
 * This macro creates a wrapper function that automatically registers thread spawn and exit
 * events with the deadlock detector. Use this for all thread functions that will interact
 * with tracked mutexes.
 *
 * @param fn_name Name of the thread function to track
 *
 * @note The original function must have the signature: void* fn_name(void*)
 *
 * @example
 * ```c
 * void* worker(void* arg) {
 *     // Thread work here
 *     return NULL;
 * }
 *
 * DEFINE_TRACKED_THREAD(worker)
 * ```
 */
#define DEFINE_TRACKED_THREAD(fn_name) \
    void* fn_name##_tracked(void* arg) { \
        unsigned long parent_tid = (uintptr_t)arg; \
        unsigned long tid = deloxide_get_thread_id(); \
        deloxide_register_thread_spawn(tid, parent_tid); \
        extern void* fn_name(void*); /* forward declare user function */ \
        void* real_arg = (void*)(uintptr_t)parent_tid; /* unwrap real argument */ \
        void* ret = fn_name(real_arg); \
        deloxide_register_thread_exit(tid); \
        return ret; \
    }

/**
 * @brief Create a tracked thread
 *
 * This macro creates a pthread while ensuring it is properly tracked by the deadlock detector.
 * Always use this instead of pthread_create() for threads that will interact with tracked mutexes.
 *
 * @param thread_var The pthread_t variable to initialize
 * @param original_fn The thread function to run (must be defined with DEFINE_TRACKED_THREAD)
 * @param real_arg The argument to pass to the thread function
 *
 * @example
 * ```c
 * pthread_t thread;
 * CREATE_TRACKED_THREAD(thread, worker, argument);
 * ```
 */
#define CREATE_TRACKED_THREAD(thread_var, original_fn, real_arg) do { \
    pthread_create(&(thread_var), NULL, original_fn##_tracked, (void*)(uintptr_t)(real_arg)); \
} while(0)


/**
 * @brief Lock a tracked mutex with automatic thread ID
 *
 * This macro locks a mutex while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_lock().
 *
 * @param mutex_ptr Pointer to the mutex to lock
 *
 * @example
 * ```c
 * LOCK(mutex);
 * // Critical section
 * UNLOCK(mutex);
 * ```
 */
#define LOCK(mutex_ptr) do { \
    if (deloxide_lock((mutex_ptr)) != 0) { \
        fprintf(stderr, "Failed to lock mutex\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Unlock a tracked mutex with automatic thread ID
 *
 * This macro unlocks a mutex while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_unlock().
 *
 * @param mutex_ptr Pointer to the mutex to unlock
 *
 * @example
 * ```c
 * LOCK(mutex);
 * // Critical section
 * UNLOCK(mutex);
 * ```
 */
#define UNLOCK(mutex_ptr) do { \
    if (deloxide_unlock((mutex_ptr)) != 0) { \
        fprintf(stderr, "Failed to unlock mutex\n"); \
        exit(1); \
    } \
} while(0)


/*
 * --- Core Deloxide FFI API ---
 */

/**
 * @brief Initialize the deadlock detector.
 *
 * This function must be called before any other Deloxide functions.
 * It sets up the deadlock detection engine and configures logging and callback options.
 *
 * @param log_path Path to the log file as a null-terminated UTF-8 string,
 *                 or NULL to disable logging entirely.
 *
 * @param callback Function to call when a deadlock is detected. It receives a null-terminated
 *                 JSON string containing detailed information about the deadlock, or NULL
 *                 to disable the callback.
 *
 *                 The JSON string passed to the callback has the following format:
 *                 {
 *                   "thread_cycle": [<thread_id_1>, <thread_id_2>, ...],
 *                   "thread_waiting_for_locks": [[<thread_id>, <lock_id>], ...],
 *                   "timestamp": "<ISO-8601 timestamp>"
 *                 }
 *
 * @return  0 on success
 *          1 if already initialized
 *         -1 if log_path contains invalid UTF-8
 *         -2 if logger initialization failed
 */
int deloxide_init(const char* log_path, void (*callback)(const char* json_info));

/**
 * @brief Check if a deadlock has been detected.
 *
 * This function can be called to poll for deadlock status without using a callback.
 *
 * @return 1 if a deadlock was detected, 0 otherwise.
 */
int deloxide_is_deadlock_detected();

/**
 * @brief Reset the deadlock detected flag.
 *
 * This allows the detector to report future deadlocks after one has been handled.
 * Call this after you've processed a deadlock notification if you want to continue
 * monitoring for additional deadlocks.
 */
void deloxide_reset_deadlock_flag();

/**
 * @brief Check if logging is currently enabled.
 *
 * @return 1 if logging is enabled, 0 if disabled.
 */
int deloxide_is_logging_enabled();

/**
 * @brief Create a new tracked mutex.
 *
 * Creates a mutex that will be tracked by the deadlock detector.
 * The current thread will be registered as the creator of this mutex.
 * When the creator thread exits, the mutex will be automatically destroyed
 * if no other thread is using it.
 *
 * @return Opaque pointer to the mutex, or NULL on allocation failure.
 */
void* deloxide_create_mutex();

/**
 * @brief Create a new tracked mutex with a specified creator thread.
 *
 * Similar to deloxide_create_mutex(), but allows specifying which thread
 * should be considered the "owner" of the mutex for resource tracking purposes.
 *
 * The specified thread will be registered as the creator of this mutex.
 * When the creator thread exits, the mutex will be automatically destroyed
 * if no other thread is using it.
 *
 * @param creator_thread_id ID of the thread to register as the creator.
 *
 * @return Opaque pointer to the mutex, or NULL on allocation failure.
 */
void* deloxide_create_mutex_with_creator(unsigned long creator_thread_id);

/**
 * @brief Destroy a tracked mutex.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @note After calling this function, the mutex pointer must not be used again.
 */
void deloxide_destroy_mutex(void* mutex);

/**
 * @brief Lock a tracked mutex.
 *
 * Attempts to acquire the lock on a mutex while tracking the operation
 * for deadlock detection.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @return  0 on success
 *         -1 if mutex is NULL
 */
int deloxide_lock(void* mutex);

/**
 * @brief Unlock a tracked mutex.
 *
 * Releases a lock on a mutex while tracking the operation for deadlock detection.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @return  0 on success
 *         -1 if mutex is NULL
 */
int deloxide_unlock(void* mutex);

/**
 * @brief Register a thread spawn with the deadlock detector.
 *
 * This function should be called when a new thread is created in your application.
 * It establishes the parent-child relationship between threads for proper resource tracking.
 *
 * @note You generally shouldn't need to call this directly if you use the CREATE_TRACKED_THREAD macro.
 *
 * @param thread_id ID of the newly spawned thread.
 * @param parent_id ID of the parent thread that created this thread, or 0 for no parent.
 *
 * @return 0 on success.
 */
int deloxide_register_thread_spawn(unsigned long thread_id, unsigned long parent_id);

/**
 * @brief Register a thread exit with the deadlock detector.
 *
 * This function should be called when a thread is about to exit.
 * It ensures proper cleanup of resources owned by the thread.
 *
 * @note You generally shouldn't need to call this directly if you use the CREATE_TRACKED_THREAD macro.
 *
 * @param thread_id ID of the exiting thread.
 *
 * @return 0 on success.
 */
int deloxide_register_thread_exit(unsigned long thread_id);

/**
 * @brief Get a unique identifier for the current thread.
 *
 * This ID should be used when calling lock/unlock functions.
 *
 * @return A unique thread ID as an unsigned long.
 */
unsigned long deloxide_get_thread_id();

/**
 * @brief Get the creator thread ID of a mutex.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @return The thread ID of the creator thread, or 0 if the mutex is NULL.
 */
unsigned long deloxide_get_mutex_creator(void* mutex);

/**
 * @brief Opens a browser window to showcase the given log data.
 *
 * This function processes the log file and sends it to the Deloxide visualization
 * server, opening a browser window to display the thread-lock relationships.
 *
 * @param log_path Path to the log file as a null-terminated UTF-8 string.
 *
 * @return  0 on success,
 *         -1 if log_path is NULL or contains invalid UTF-8,
 *         -2 if the showcase operation failed.
 */
int deloxide_showcase(const char* log_path);

/**
 * @brief Opens a browser window to showcase the currently active log data.
 *
 * This function uses the log file that was specified in deloxide_init().
 * It's a convenience wrapper around deloxide_showcase() that uses the current log file.
 *
 * @return  0 on success,
 *         -1 if no active log file exists,
 *         -2 if the showcase operation failed.
 */
int deloxide_showcase_current();

#endif /* DELOXIDE_H */