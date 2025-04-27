/*
 * @file deloxide.h
 * @brief C/C++ API for the Rust deloxide deadlock detection library.
 */

#ifndef DELOXIDE_H
#define DELOXIDE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>

/*
 * @brief Initialize the deadlock detector.
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

/*
 * @brief Check if a deadlock has been detected.
 *
 * @return 1 if a deadlock was detected, 0 otherwise.
 */
int deloxide_is_deadlock_detected();

/*
 * @brief Reset the deadlock detected flag.
 *
 * This allows the detector to report future deadlocks after one has been handled.
 */
void deloxide_reset_deadlock_flag();

/*
 * @brief Check if logging is currently enabled.
 *
 * @return 1 if logging is enabled, 0 if disabled.
 */
int deloxide_is_logging_enabled();

/*
 * @brief Create a new tracked mutex.
 *
 * The current thread will be registered as the creator of this mutex.
 * When the creator thread exits, the mutex will be automatically destroyed
 * if no other thread is using it.
 *
 * @return Opaque pointer to the mutex, or NULL on allocation failure.
 */
void* deloxide_create_mutex();

/*
 * @brief Create a new tracked mutex with a specified creator thread.
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

/*
 * @brief Destroy a tracked mutex.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @note After calling this function, the mutex pointer must not be used again.
 */
void deloxide_destroy_mutex(void* mutex);

/*
 * @brief Lock a tracked mutex.
 *
 * @param mutex     Pointer to a mutex created with deloxide_create_mutex.
 * @param thread_id Thread ID obtained from deloxide_get_thread_id().
 *
 * @return  0 on success
 *         -1 if mutex is NULL
 *         -2 if lock acquisition failed
 */
int deloxide_lock(void* mutex, unsigned long thread_id);

/*
 * @brief Unlock a tracked mutex.
 *
 * @param mutex     Pointer to a mutex created with deloxide_create_mutex.
 * @param thread_id Thread ID obtained from deloxide_get_thread_id().
 *
 * @return  0 on success
 *         -1 if mutex is NULL
 */
int deloxide_unlock(void* mutex, unsigned long thread_id);

/*
 * @brief Register a thread spawn with the deadlock detector.
 *
 * This function should be called when a new thread is created in your application.
 * It establishes the parent-child relationship between threads for proper resource tracking.
 *
 * @param thread_id ID of the newly spawned thread.
 * @param parent_id ID of the parent thread that created this thread, or 0 for no parent.
 *
 * @return 0 on success.
 */
int deloxide_register_thread_spawn(unsigned long thread_id, unsigned long parent_id);

/*
 * @brief Register a thread exit with the deadlock detector.
 *
 * This function should be called when a thread is about to exit.
 * It ensures proper cleanup of resources owned by the thread.
 *
 * @param thread_id ID of the exiting thread.
 *
 * @return 0 on success.
 */
int deloxide_register_thread_exit(unsigned long thread_id);

/*
 * @brief Get a unique identifier for the current thread.
 *
 * This ID should be used when calling lock/unlock functions.
 *
 * @return A unique thread ID as an unsigned long.
 */
unsigned long deloxide_get_thread_id();

/*
 * @brief Get the creator thread ID of a mutex.
 *
 * @param mutex Pointer to a mutex created with deloxide_create_mutex.
 *
 * @return The thread ID of the creator thread, or 0 if the mutex is NULL.
 */
unsigned long deloxide_get_mutex_creator(void* mutex);

/*
 * @brief Opens a browser window to showcase the given log data.
 *
 * @param log_path Path to the log file as a null-terminated UTF-8 string.
 *
 * @return  0 on success,
 *         -1 if log_path is NULL or contains invalid UTF-8,
 *         -2 if the showcase operation failed.
 */
int deloxide_showcase(const char* log_path);

/*
 * @brief Opens a browser window to showcase the currently active log data.
 *
 * This function uses the log file that was specified in deloxide_init().
 *
 * @return  0 on success,
 *         -1 if no active log file exists,
 *         -2 if the showcase operation failed.
 */
int deloxide_showcase_current();

#ifdef __cplusplus
}
#endif

#endif /* DELOXIDE_H */