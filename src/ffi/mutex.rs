use crate::core::detector::mutex::create_mutex;
use crate::ffi::FFI_GUARD;
use crate::{Mutex, ThreadId};
use std::ffi::c_void;
use std::os::raw::c_int;

/// Create a new tracked mutex.
///
/// Creates a mutex that will be tracked by the deadlock detector. The current
/// thread will be recorded as the creator of this mutex.
///
/// # Returns
/// * Void pointer to the mutex, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer to a heap allocation and must be freed by `deloxide_destroy_mutex`.
/// - Any usage from C must ensure not to free or move the returned pointer by other means.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_mutex() -> *mut c_void {
    let mutex = Box::new(Mutex::new(()));
    Box::into_raw(mutex) as *mut c_void
}

/// Create a new tracked mutex with specified creator thread ID.
///
/// Similar to deloxide_create_mutex(), but allows specifying which thread
/// should be considered the "owner" for resource tracking purposes.
///
/// # Arguments
/// * `creator_thread_id` - ID of the thread to be registered as the creator of this mutex.
///
/// # Returns
/// * Void pointer to the mutex, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer to a heap allocation and must be freed by `deloxide_destroy_mutex`.
/// - Any usage from C must ensure not to free or move the returned pointer by other means.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_mutex_with_creator(
    creator_thread_id: usize,
) -> *mut c_void {
    let mutex = Box::new(Mutex::new(()));

    // Register the specified thread as the creator
    create_mutex(mutex.id(), Some(creator_thread_id as ThreadId));

    Box::into_raw(mutex) as *mut c_void
}

/// Destroy a tracked mutex.
///
/// Frees the memory associated with a tracked mutex and removes it from
/// the deadlock detector's tracking.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
///
/// # Safety
/// - The caller must ensure that `mutex` is not used by any thread after this function is called.
/// - The pointer must be one previously obtained from `deloxide_create_mutex` (i.e., it must not be a stack pointer or null pointer).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_destroy_mutex(mutex: *mut c_void) {
    if !mutex.is_null() {
        unsafe {
            drop(Box::from_raw(mutex as *mut Mutex<()>));
        }
    }
}

/// Lock a tracked mutex.
///
/// Attempts to acquire the lock on a mutex while tracking the operation
/// for deadlock detection.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
///
/// # Returns
/// * `0` on success
/// * `-1` if the mutex pointer is NULL
///
/// # Safety
/// - The caller must pass a valid pointer to a `Mutex<()>`.
/// - The lock is re-entrant in the sense of C code, but you must not call `deloxide_lock` twice on the same mutex from the same thread without calling `deloxide_unlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_lock_mutex(mutex: *mut c_void) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const Mutex<()>);
        let guard = mutex_ref.lock();

        #[allow(clippy::missing_transmute_annotations)]
        FFI_GUARD.with(|map| {
            map.borrow_mut().insert(mutex, std::mem::transmute(guard));
        });
    }

    0
}

/// Unlock a tracked mutex.
///
/// Releases a lock on a mutex while tracking the operation for deadlock detection.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
///
/// # Returns
/// * `0` on success
/// * `-1` if the mutex pointer is NULL
///
/// # Safety
/// - The pointer must be valid (i.e., a previously created `Mutex<()>`).
/// - The mutex must have been previously locked by the current thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_unlock_mutex(mutex: *mut c_void) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    // Drop the guard we stashed above; this actually unlocks the Mutex
    FFI_GUARD.with(|map| {
        map.borrow_mut().remove(&mutex);
    });

    0
}

/// Get the creator thread ID of a mutex.
///
/// Returns the ID of the thread that created the specified mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
///
/// # Returns
/// * Thread ID of the creator thread, or 0 if the mutex is NULL
///
/// # Safety
/// - The caller must pass a valid pointer to a `Mutex<()>`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_mutex_creator(mutex: *mut c_void) -> usize {
    if mutex.is_null() {
        return 0;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const Mutex<()>);
        mutex_ref.creator_thread_id()
    }
}
