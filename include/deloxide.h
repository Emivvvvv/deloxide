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
 * 3. Use the LOCK_MUTEX() and UNLOCK_MUTEX() macros to operate on mutexes
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
 *     LOCK_MUTEX(mutex);
 *     // Critical section
 *     UNLOCK_MUTEX(mutex);
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
 *            LOCK_MUTEX(mutex);
 *            // do work
 *            UNLOCK_MUTEX(mutex);
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
 *        LOCK_MUTEX(mutex);
 *        ... critical section ...
 *        UNLOCK_MUTEX(mutex);
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#if !defined(_WIN32)
#include <pthread.h>
#endif

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
#if !defined(_WIN32)
typedef struct {
    uintptr_t parent_tid;
    void* user_arg;
} deloxide_thread_arg_t;

#define DEFINE_TRACKED_THREAD(fn_name) \
    void* fn_name##_tracked(void* _arg) { \
        deloxide_thread_arg_t a = *(deloxide_thread_arg_t*)_arg; \
        free(_arg); \
        uintptr_t tid = deloxide_get_thread_id(); \
        deloxide_register_thread_spawn(tid, a.parent_tid); \
        extern void* fn_name(void*); /* forward declare user function */ \
        void* ret = fn_name(a.user_arg); \
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
    deloxide_thread_arg_t* a = (deloxide_thread_arg_t*)malloc(sizeof(deloxide_thread_arg_t)); \
    a->parent_tid = deloxide_get_thread_id(); \
    a->user_arg = (void*)(real_arg); \
    pthread_create(&(thread_var), NULL, original_fn##_tracked, (void*)a); \
} while(0)
#else
#define DEFINE_TRACKED_THREAD(fn_name) /* not available on Windows */
#define CREATE_TRACKED_THREAD(thread_var, original_fn, real_arg) /* not available on Windows */
#endif


/**
 * @brief Lock a tracked mutex with automatic thread ID
 *
 * This macro locks a mutex while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_lock_mutex().
 *
 * @param mutex_ptr Pointer to the mutex to lock
 *
 * @example
 * ```c
 * LOCK_MUTEX(mutex);
 * // Critical section
 * UNLOCK_MUTEX(mutex);
 * ```
 */
#define LOCK_MUTEX(mutex_ptr) do { \
    if (deloxide_lock_mutex((mutex_ptr)) != 0) { \
        fprintf(stderr, "Failed to lock mutex\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Unlock a tracked mutex with automatic thread ID
 *
 * This macro unlocks a mutex while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_unlock_mutex().
 *
 * @param mutex_ptr Pointer to the mutex to unlock
 *
 * @example
 * ```c
 * LOCK_MUTEX(mutex);
 * // Critical section
 * UNLOCK_MUTEX(mutex);
 * ```
 */
#define UNLOCK_MUTEX(mutex_ptr) do { \
    if (deloxide_unlock_mutex((mutex_ptr)) != 0) { \
        fprintf(stderr, "Failed to unlock mutex\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Lock a tracked RwLock for reading.
 *
 * This macro locks an RwLock in read mode while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_rw_lock_read().
 *
 * @param rwlock_ptr Pointer to the RwLock to lock for reading
 */
#define RWLOCK_READ(rwlock_ptr) do { \
    if (deloxide_rw_lock_read((rwlock_ptr)) != 0) { \
        fprintf(stderr, "Failed to acquire RwLock read lock\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Unlock a tracked RwLock from read mode.
 *
 * This macro unlocks an RwLock previously locked for reading.
 * Always use this instead of directly calling deloxide_rw_unlock_read().
 *
 * @param rwlock_ptr Pointer to the RwLock to unlock from reading
 */
#define RWUNLOCK_READ(rwlock_ptr) do { \
    if (deloxide_rw_unlock_read((rwlock_ptr)) != 0) { \
        fprintf(stderr, "Failed to release RwLock read lock\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Lock a tracked RwLock for writing.
 *
 * This macro locks an RwLock in write mode while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_rw_lock_write().
 *
 * @param rwlock_ptr Pointer to the RwLock to lock for writing
 */
#define RWLOCK_WRITE(rwlock_ptr) do { \
    if (deloxide_rw_lock_write((rwlock_ptr)) != 0) { \
        fprintf(stderr, "Failed to acquire RwLock write lock\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Unlock a tracked RwLock from write mode.
 *
 * This macro unlocks an RwLock previously locked for writing.
 * Always use this instead of directly calling deloxide_rw_unlock_write().
 *
 * @param rwlock_ptr Pointer to the RwLock to unlock from writing
 */
#define RWUNLOCK_WRITE(rwlock_ptr) do { \
    if (deloxide_rw_unlock_write((rwlock_ptr)) != 0) { \
        fprintf(stderr, "Failed to release RwLock write lock\n"); \
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
void* deloxide_create_mutex_with_creator(uintptr_t creator_thread_id);

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
int deloxide_lock_mutex(void* mutex);

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
int deloxide_unlock_mutex(void* mutex);

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
int deloxide_register_thread_spawn(uintptr_t thread_id, uintptr_t parent_id);

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
int deloxide_register_thread_exit(uintptr_t thread_id);

/**
 * @brief Get a unique identifier for the current thread.
 *
 * This ID should be used when calling lock/unlock functions.
 *
 * @return A unique thread ID as an unsigned long.
 */
uintptr_t deloxide_get_thread_id();

/**
 * @brief Get the creator thread ID of a mutex.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @return The thread ID of the creator thread, or 0 if the mutex is NULL.
 */
uintptr_t deloxide_get_mutex_creator(void* mutex);

/**
 * @brief Flush all pending log entries to disk.
 *
 * This function forces all buffered log entries to be written to disk.
 * It should be called before processing the log file or showcasing to
 * ensure that all logged events are visible.
 *
 * @return  0 on success,
 *         -1 if flushing failed.
 */
int deloxide_flush_logs();

/**
 * @brief Opens a browser window to showcase the given log data.
 *
 * This function flushes pending log entries, processes the log file, and sends
 * it to the Deloxide visualization server, opening a browser window to display
 * the thread-lock relationships.
 *
 * @param log_path Path to the log file as a null-terminated UTF-8 string.
 *
 * @return  0 on success,
 *         -1 if log_path is NULL or contains invalid UTF-8,
 *         -2 if the showcase operation failed,
 *         -3 if flushing failed.
 */
int deloxide_showcase(const char* log_path);

/**
 * @brief Opens a browser window to showcase the currently active log data.
 *
 * This function ensures all buffered log entries are flushed to disk before
 * showcasing the log file that was specified in deloxide_init().
 *
 * @return  0 on success,
 *         -1 if no active log file exists,
 *         -2 if the showcase operation failed,
 *         -3 if flushing failed.
 */
int deloxide_showcase_current();

/*
 * --- Stress Testing API ---
 *
 * These functions provide stress testing capabilities to increase the
 * probability of deadlock manifestation during testing. They are only
 * available when Deloxide is compiled with the "stress-test" feature.
 */

/**
 * @brief Enable stress testing with random preemptions.
 *
 * This function enables stress testing with random preemptions before lock
 * acquisitions to increase deadlock probability. It should be called before
 * deloxide_init().
 *
 * @param probability Probability of preemption (0.0-1.0)
 * @param min_delay_us Minimum delay duration in microseconds
 * @param max_delay_us Maximum delay duration in microseconds
 *
 * @return 0 on success, 1 if already initialized, -1 if stress-test feature not enabled
 *
 * @note This function is only available when Deloxide is compiled with the "stress-test" feature.
 */
int deloxide_enable_random_stress(double probability, unsigned long min_delay_us, unsigned long max_delay_us);

/**
 * @brief Enable stress testing with component-based delays.
 *
 * This function enables stress testing with targeted delays based on lock
 * graph analysis to increase deadlock probability. It should be called before
 * deloxide_init().
 *
 * @param min_delay_us Minimum delay duration in microseconds
 * @param max_delay_us Maximum delay duration in microseconds
 *
 * @return 0 on success, 1 if already initialized, -1 if stress-test feature not enabled
 *
 * @note This function is only available when Deloxide is compiled with the "stress-test" feature.
 */
int deloxide_enable_component_stress(unsigned long min_delay_us, unsigned long max_delay_us);

/**
 * @brief Disable stress testing.
 *
 * This function disables any previously enabled stress testing mode.
 * It should be called before deloxide_init().
 *
 * @return 0 on success, 1 if already initialized, -1 if stress-test feature not enabled
 *
 * @note This function is only available when Deloxide is compiled with the "stress-test" feature.
 */
int deloxide_disable_stress();

/**
 * @brief Create a new tracked RwLock.
 *
 * Creates a RwLock that will be tracked by the deadlock detector.
 * The current thread will be registered as the creator of this RwLock.
 *
 * @return Opaque pointer to the RwLock, or NULL on allocation failure.
 */
void* deloxide_create_rwlock();

/**
 * @brief Create a new tracked RwLock with a specified creator thread.
 *
 * Allows specifying which thread should be considered the "owner" for resource tracking.
 *
 * @param creator_thread_id ID of the thread to register as the creator.
 * @return Opaque pointer to the RwLock, or NULL on allocation failure.
 */
void* deloxide_create_rwlock_with_creator(uintptr_t creator_thread_id);

/**
 * @brief Destroy a tracked RwLock.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @note After calling this function, the RwLock pointer must not be used again.
 */
void deloxide_destroy_rwlock(void* rwlock);

/**
 * @brief Lock a tracked RwLock for reading.
 *
 * Attempts to acquire a shared (read) lock on a RwLock for deadlock detection.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @return  0 on success, -1 if rwlock is NULL
 */
int deloxide_rw_lock_read(void* rwlock);

/**
 * @brief Unlock a tracked RwLock from reading.
 *
 * Releases a shared (read) lock previously acquired on a RwLock.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @return  0 on success, -1 if rwlock is NULL
 */
int deloxide_rw_unlock_read(void* rwlock);

/**
 * @brief Lock a tracked RwLock for writing.
 *
 * Attempts to acquire an exclusive (write) lock on a RwLock for deadlock detection.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @return  0 on success, -1 if rwlock is NULL
 */
int deloxide_rw_lock_write(void* rwlock);

/**
 * @brief Unlock a tracked RwLock from writing.
 *
 * Releases an exclusive (write) lock previously acquired on a RwLock.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @return  0 on success, -1 if rwlock is NULL
 */
int deloxide_rw_unlock_write(void* rwlock);

/**
 * @brief Get the creator thread ID of a RwLock.
 *
 * @param rwlock Pointer to a RwLock created with deloxide_create_rwlock.
 * @return The thread ID of the creator thread, or 0 if the RwLock is NULL.
 */
uintptr_t deloxide_get_rwlock_creator(void* rwlock);

/**
 * @brief Create a new tracked condition variable.
 *
 * Creates a condition variable that will be tracked by the deadlock detector.
 * The current thread will be registered as the creator of this condition variable.
 *
 * @return Opaque pointer to the condition variable, or NULL on allocation failure.
 */
void* deloxide_create_condvar();

/**
 * @brief Create a new tracked condition variable with a specified creator thread.
 *
 * Allows specifying which thread should be considered the "owner" for resource tracking.
 *
 * @param creator_thread_id ID of the thread to register as the creator.
 * @return Opaque pointer to the condition variable, or NULL on allocation failure.
 */
void* deloxide_create_condvar_with_creator(uintptr_t creator_thread_id);

/**
 * @brief Destroy a tracked condition variable.
 *
 * @param condvar Pointer to a condition variable created with deloxide_create_condvar.
 * @note After calling this function, the condition variable pointer must not be used again.
 */
void deloxide_destroy_condvar(void* condvar);

/**
 * @brief Wait on a condition variable.
 *
 * This function atomically releases the associated mutex and waits for the condition variable
 * to be signaled. When the function returns, the mutex will be re-acquired.
 *
 * @param condvar Pointer to a condition variable created with deloxide_create_condvar.
 * @param mutex Pointer to a mutex that is currently locked by this thread.
 * @return  0 on success
 *         -1 if condvar is NULL
 *         -2 if mutex is NULL  
 *         -3 if mutex is not currently held by this thread
 *         -4 if wait operation failed
 */
int deloxide_condvar_wait(void* condvar, void* mutex);

/**
 * @brief Wait on a condition variable with a timeout.
 *
 * This function atomically releases the associated mutex and waits for the condition variable
 * to be signaled or until the timeout expires. When the function returns, the mutex will be re-acquired.
 *
 * @param condvar Pointer to a condition variable created with deloxide_create_condvar.
 * @param mutex Pointer to a mutex that is currently locked by this thread.
 * @param timeout_ms Timeout in milliseconds. The function will wait at most this long.
 * @return  0 on success (condition variable was signaled)
 *          1 on timeout
 *         -1 if condvar is NULL
 *         -2 if mutex is NULL
 *         -3 if mutex is not currently held by this thread
 *         -4 if wait operation failed
 */
int deloxide_condvar_wait_timeout(void* condvar, void* mutex, unsigned long timeout_ms);

/**
 * @brief Signal one thread waiting on the condition variable.
 *
 * This function wakes up one thread that is waiting on the condition variable.
 * If no threads are waiting, this function has no effect.
 *
 * @param condvar Pointer to a condition variable created with deloxide_create_condvar.
 * @return  0 on success
 *         -1 if condvar is NULL
 */
int deloxide_condvar_notify_one(void* condvar);

/**
 * @brief Signal all threads waiting on the condition variable.
 *
 * This function wakes up all threads that are waiting on the condition variable.
 * If no threads are waiting, this function has no effect.
 *
 * @param condvar Pointer to a condition variable created with deloxide_create_condvar.
 * @return  0 on success
 *         -1 if condvar is NULL
 */
int deloxide_condvar_notify_all(void* condvar);

/**
 * @brief Wait on a tracked condition variable.
 *
 * This macro waits on a condition variable while ensuring the operation is tracked by the deadlock detector.
 * The associated mutex will be automatically unlocked during wait and re-locked when signaled.
 * Always use this instead of directly calling deloxide_condvar_wait().
 *
 * @param condvar_ptr Pointer to the condition variable to wait on
 * @param mutex_ptr Pointer to the mutex that must be currently locked
 */
#define CONDVAR_WAIT(condvar_ptr, mutex_ptr) do { \
    int result = deloxide_condvar_wait((condvar_ptr), (mutex_ptr)); \
    if (result != 0) { \
        fprintf(stderr, "Failed to wait on condition variable (error %d)\n", result); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Wait on a tracked condition variable with timeout.
 *
 * This macro waits on a condition variable with a timeout while ensuring the operation is tracked.
 * The associated mutex will be automatically unlocked during wait and re-locked when signaled or timeout occurs.
 * Always use this instead of directly calling deloxide_condvar_wait_timeout().
 *
 * @param condvar_ptr Pointer to the condition variable to wait on
 * @param mutex_ptr Pointer to the mutex that must be currently locked
 * @param timeout_ms Timeout in milliseconds
 * @return The result from deloxide_condvar_wait_timeout (0=success, 1=timeout, <0=error)
 */
#define CONDVAR_WAIT_TIMEOUT(condvar_ptr, mutex_ptr, timeout_ms) \
    deloxide_condvar_wait_timeout((condvar_ptr), (mutex_ptr), (timeout_ms))

/**
 * @brief Signal one thread waiting on a condition variable.
 *
 * This macro signals one waiting thread while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_condvar_notify_one().
 *
 * @param condvar_ptr Pointer to the condition variable to signal
 */
#define CONDVAR_NOTIFY_ONE(condvar_ptr) do { \
    if (deloxide_condvar_notify_one((condvar_ptr)) != 0) { \
        fprintf(stderr, "Failed to notify one on condition variable\n"); \
        exit(1); \
    } \
} while(0)

/**
 * @brief Signal all threads waiting on a condition variable.
 *
 * This macro signals all waiting threads while ensuring the operation is tracked by the deadlock detector.
 * Always use this instead of directly calling deloxide_condvar_notify_all().
 *
 * @param condvar_ptr Pointer to the condition variable to signal
 */
#define CONDVAR_NOTIFY_ALL(condvar_ptr) do { \
    if (deloxide_condvar_notify_all((condvar_ptr)) != 0) { \
        fprintf(stderr, "Failed to notify all on condition variable\n"); \
        exit(1); \
    } \
} while(0)

#endif /* DELOXIDE_H */