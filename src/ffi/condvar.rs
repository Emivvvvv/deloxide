use crate::core::locks::condvar::Condvar;
use crate::core::locks::mutex::MutexGuard;
use crate::core::detector::condvar::on_condvar_create;
use crate::ffi::FFI_GUARD;
use std::cell::RefCell;
use std::ffi::{c_int, c_ulong, c_void};
use std::time::Duration;

// Each thread can hold condition variable wait state
thread_local! {
    static FFI_CONDVAR_WAIT_STATE: RefCell<Option<(*mut c_void, *mut c_void)>> = const {RefCell::new(None)};
}

/// Create a new tracked condition variable.
///
/// Creates a condition variable that will be tracked by the deadlock detector.
/// The current thread will be registered as the creator of this condition variable.
///
/// # Returns
/// * Void pointer to the condition variable, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer to a heap allocation and must be freed by `deloxide_destroy_condvar`.
/// - Any usage from C must ensure not to free or move the returned pointer by other means.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_condvar() -> *mut c_void {
    let condvar = Box::new(Condvar::new());
    Box::into_raw(condvar) as *mut c_void
}

/// Create a new tracked condition variable with specified creator thread ID.
///
/// Similar to deloxide_create_condvar(), but allows specifying which thread
/// should be considered the "owner" for resource tracking purposes.
///
/// # Arguments
/// * `creator_thread_id` - ID of the thread to be registered as the creator of this condition variable.
///
/// # Returns
/// * Void pointer to the condition variable, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer to a heap allocation and must be freed by `deloxide_destroy_condvar`.
/// - Any usage from C must ensure not to free or move the returned pointer by other means.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_condvar_with_creator(
    _creator_thread_id: c_ulong,
) -> *mut c_void {
    let condvar = Box::new(Condvar::new());

    // Register the specified thread as the creator
    // Note: The condvar detector doesn't currently support custom creator threads
    // so we just create the condvar normally
    on_condvar_create(condvar.id());

    Box::into_raw(condvar) as *mut c_void
}

/// Destroy a tracked condition variable.
///
/// # Arguments
/// * `condvar` - Pointer to a condition variable created with `deloxide_create_condvar`.
///
/// # Safety
/// - The pointer must be valid and created with deloxide_create_condvar.
/// - After calling this function, the condition variable pointer must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_destroy_condvar(condvar: *mut c_void) {
    if condvar.is_null() {
        return;
    }
    let _condvar: Box<Condvar> = unsafe { Box::from_raw(condvar as *mut Condvar) };
    // Drop happens automatically
}

/// Wait on a condition variable.
///
/// This function atomically releases the associated mutex and waits for the condition variable
/// to be signaled. When the function returns, the mutex will be re-acquired.
///
/// # Arguments
/// * `condvar` - Pointer to a condition variable created with `deloxide_create_condvar`.
/// * `mutex` - Pointer to a mutex that is currently locked by this thread.
///
/// # Returns
/// * 0 on success
/// * -1 if condvar is NULL
/// * -2 if mutex is NULL
/// * -3 if mutex is not currently held by this thread
/// * -4 if wait operation failed
///
/// # Safety
/// - Both pointers must be valid and created with appropriate deloxide functions.
/// - The mutex must be currently locked by the calling thread.
/// - The mutex will be automatically unlocked during wait and re-locked on return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_condvar_wait(
    condvar: *mut c_void,
    mutex: *mut c_void,
) -> c_int {
    if condvar.is_null() {
        return -1;
    }
    if mutex.is_null() {
        return -2;
    }

    let condvar_ref = unsafe { &*(condvar as *const Condvar) };

    // Get the current mutex guard for this specific mutex from thread-local storage
    let mut guard = match FFI_GUARD.with(|map| map.borrow_mut().remove(&mutex)) {
        Some(guard) => guard,
        None => return -3, // Mutex not held by this thread
    };

    // Store the condvar and mutex pointers for cleanup
    FFI_CONDVAR_WAIT_STATE.with(|cell| {
        *cell.borrow_mut() = Some((condvar, mutex));
    });

    // Perform the wait operation
    condvar_ref.wait(&mut guard);

    // Store the guard back in thread-local storage for this mutex
    FFI_GUARD.with(|map| {
        map.borrow_mut().insert(mutex, guard);
    });

    // Clear wait state
    FFI_CONDVAR_WAIT_STATE.with(|cell| {
        *cell.borrow_mut() = None;
    });

    0
}

/// Wait on a condition variable with a timeout.
///
/// This function atomically releases the associated mutex and waits for the condition variable
/// to be signaled or until the timeout expires. When the function returns, the mutex will be re-acquired.
///
/// # Arguments
/// * `condvar` - Pointer to a condition variable created with `deloxide_create_condvar`.
/// * `mutex` - Pointer to a mutex that is currently locked by this thread.
/// * `timeout_ms` - Timeout in milliseconds. The function will wait at most this long.
///
/// # Returns
/// * 0 on success (condition variable was signaled)
/// * 1 on timeout
/// * -1 if condvar is NULL
/// * -2 if mutex is NULL
/// * -3 if mutex is not currently held by this thread
/// * -4 if wait operation failed
///
/// # Safety
/// - Both pointers must be valid and created with appropriate deloxide functions.
/// - The mutex must be currently locked by the calling thread.
/// - The mutex will be automatically unlocked during wait and re-locked on return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_condvar_wait_timeout(
    condvar: *mut c_void,
    mutex: *mut c_void,
    timeout_ms: c_ulong,
) -> c_int {
    if condvar.is_null() {
        return -1;
    }
    if mutex.is_null() {
        return -2;
    }

    let condvar_ref = unsafe { &*(condvar as *const Condvar) };

    // Get the current mutex guard for this specific mutex from thread-local storage
    let mut guard = match FFI_GUARD.with(|map| map.borrow_mut().remove(&mutex)) {
        Some(guard) => guard,
        None => return -3, // Mutex not held by this thread
    };

    // Store the condvar and mutex pointers for cleanup
    FFI_CONDVAR_WAIT_STATE.with(|cell| {
        *cell.borrow_mut() = Some((condvar, mutex));
    });

    // Perform the wait operation with timeout
    let timeout = Duration::from_millis(timeout_ms as u64);
    let timed_out = condvar_ref.wait_timeout(&mut guard, timeout);

    // Store the guard back in thread-local storage for this mutex
    FFI_GUARD.with(|map| {
        map.borrow_mut().insert(mutex, guard);
    });

    // Clear wait state
    FFI_CONDVAR_WAIT_STATE.with(|cell| {
        *cell.borrow_mut() = None;
    });

    if timed_out {
        1 // Timeout
    } else {
        0 // Success
    }
}

/// Signal one thread waiting on the condition variable.
///
/// This function wakes up one thread that is waiting on the condition variable.
/// If no threads are waiting, this function has no effect.
///
/// # Arguments
/// * `condvar` - Pointer to a condition variable created with `deloxide_create_condvar`.
///
/// # Returns
/// * 0 on success
/// * -1 if condvar is NULL
///
/// # Safety
/// - The pointer must be valid and created with deloxide_create_condvar.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_condvar_notify_one(condvar: *mut c_void) -> c_int {
    if condvar.is_null() {
        return -1;
    }

    let condvar_ref = unsafe { &*(condvar as *const Condvar) };
    condvar_ref.notify_one();
    0
}

/// Signal all threads waiting on the condition variable.
///
/// This function wakes up all threads that are waiting on the condition variable.
/// If no threads are waiting, this function has no effect.
///
/// # Arguments
/// * `condvar` - Pointer to a condition variable created with `deloxide_create_condvar`.
///
/// # Returns
/// * 0 on success
/// * -1 if condvar is NULL
///
/// # Safety
/// - The pointer must be valid and created with deloxide_create_condvar.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_condvar_notify_all(condvar: *mut c_void) -> c_int {
    if condvar.is_null() {
        return -1;
    }

    let condvar_ref = unsafe { &*(condvar as *const Condvar) };
    condvar_ref.notify_all();
    0
}