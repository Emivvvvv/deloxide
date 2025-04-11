use crate::core::logger;
use crate::core::{
    ThreadId, TrackedMutex, init_detector, on_lock_acquired, on_lock_attempt, on_lock_release,
};
use serde_json;
use std::ffi::{CStr, CString, c_void};
use std::os::raw::{c_char, c_int, c_ulong};
use std::sync::atomic::{AtomicBool, Ordering};

// Globals to track initialization state
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut DEADLOCK_DETECTED: AtomicBool = AtomicBool::new(false);

// Optional callback function provided by C code
static mut DEADLOCK_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

/// Initialize deloxide.
///
/// # Arguments
/// * `log_path` - Path to log file as a null-terminated C string, or NULL to disable logging.
/// * `callback` - Function pointer to call when a deadlock is detected, or NULL for no callback.
///
/// # Returns
/// * `0` on success
/// * `1` if detector is already initialized
/// * `-1` if the log path contains invalid UTF-8
/// * `-2` if the logger failed to initialize
///
/// # Safety
/// This function dereferences raw pointers (`log_path`) and writes to mutable global statics:
///  - The caller must ensure `log_path` is either `NULL` or a valid null-terminated string.
///  - Concurrency must be managed so that global statics (`DEADLOCK_DETECTED` and `DEADLOCK_CALLBACK`) are not mutated unsafely from multiple threads.
///  - Because this is an FFI boundary, the Rust side cannot guarantee the validity of incoming data. Callers must uphold these invariants.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_init(
    log_path: *const c_char,
    callback: Option<extern "C" fn(*const c_char)>,
) -> c_int {
    unsafe {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        // Convert C string to Rust if not NULL
        let log_path_option = if !log_path.is_null() {
            match CStr::from_ptr(log_path).to_str() {
                Ok(s) => Some(s),
                Err(_) => return -1, // Invalid UTF-8
            }
        } else {
            None // Logging is disabled
        };

        // Store callback for later use
        DEADLOCK_CALLBACK = callback;

        // Initialize with a callback that sets a flag and calls the C callback
        match logger::init_logger(log_path_option) {
            Ok(_) => {
                init_detector(|deadlock_info| {
                    #[allow(static_mut_refs)]
                    DEADLOCK_DETECTED.store(true, Ordering::SeqCst);

                    // Call C callback if provided
                    if let Some(cb) = DEADLOCK_CALLBACK {
                        // Format deadlock info as JSON
                        if let Ok(json) = serde_json::to_string(&deadlock_info) {
                            // Convert JSON to CString, then pass ptr to callback
                            if let Ok(c_str) = CString::new(json) {
                                cb(c_str.as_ptr());
                            }
                        }
                    }
                });
                INITIALIZED.store(true, Ordering::SeqCst);
                0 // Success
            }
            Err(_) => -2, // Failed to initialize logger
        }
    }
}

/// Check if a deadlock has been detected.
///
/// # Returns
/// * `1` if a deadlock was detected
/// * `0` if no deadlock has been detected
///
/// # Safety
/// This function reads from a mutable global static (`DEADLOCK_DETECTED`).
///  - The caller must ensure no data races occur when multiple threads call this function simultaneously.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_is_deadlock_detected() -> c_int {
    unsafe {
        #[allow(static_mut_refs)]
        if DEADLOCK_DETECTED.load(Ordering::SeqCst) {
            1
        } else {
            0
        }
    }
}

/// Reset the deadlock detected flag.
///
/// This allows the detector to report future deadlocks after one has been handled.
///
/// # Safety
/// This function writes to a mutable global static (`DEADLOCK_DETECTED`).
///  - The caller must ensure no data races occur when multiple threads call this function simultaneously.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_reset_deadlock_flag() {
    unsafe {
        #[allow(static_mut_refs)]
        DEADLOCK_DETECTED.store(false, Ordering::SeqCst);
    }
}

/// Check if logging is enabled.
///
/// # Returns
/// * `1` if logging is enabled
/// * `0` if logging is disabled
///
/// # Safety
/// This function is marked `unsafe` because it is part of the FFI boundary, but it only calls a safe Rust
/// function to check logging status. The caller must still respect all FFI constraints regarding function calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_is_logging_enabled() -> c_int {
    if logger::is_logging_enabled() { 1 } else { 0 }
}

/// Create a new tracked mutex.
///
/// # Returns
/// * Void pointer to the mutex, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer to a heap allocation and must be freed by `deloxide_destroy_mutex`.
/// - Any usage from C/C++ must ensure not to free or move the returned pointer by other means.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_mutex() -> *mut c_void {
    let mutex = Box::new(TrackedMutex::new(()));
    Box::into_raw(mutex) as *mut c_void
}

/// Destroy a tracked mutex.
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
            drop(Box::from_raw(mutex as *mut TrackedMutex<()>));
        }
    }
}

/// Lock a tracked mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
/// * `thread_id` - ID of the thread attempting to acquire the lock, typically from `deloxide_get_thread_id()`.
///
/// # Returns
/// * `0` on successful lock acquisition
/// * `-1` if the mutex pointer is NULL
/// * `-2` if the lock operation failed (mutex is poisoned)
///
/// # Safety
/// - The caller must pass a valid pointer to a `TrackedMutex<()>`.
/// - The caller must ensure `thread_id` matches the thread that is calling (so the deadlock detector data remains consistent).
/// - The lock is re-entrant in the sense of C code, but you must not call `deloxide_lock` twice on the same mutex from the same thread without calling `deloxide_unlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_lock(mutex: *mut c_void, thread_id: c_ulong) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const TrackedMutex<()>);

        on_lock_attempt(thread_id as ThreadId, mutex_ref.id());

        match mutex_ref.lock() {
            Ok(_) => {
                on_lock_acquired(thread_id as ThreadId, mutex_ref.id());
                0 // Success
            }
            Err(_) => -2, // Failed to acquire lock (poisoned)
        }
    }
}

/// Unlock a tracked mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with `deloxide_create_mutex`.
/// * `thread_id` - ID of the thread releasing the lock, typically from `deloxide_get_thread_id()`.
///
/// # Returns
/// * `0` on success
/// * `-1` if the mutex pointer is NULL
///
/// # Safety
/// - The caller must ensure that the mutex is currently locked by the same thread (`thread_id`).
/// - The pointer must be valid (i.e., a previously created `TrackedMutex<()>`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_unlock(mutex: *mut c_void, thread_id: c_ulong) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const TrackedMutex<()>);
        on_lock_release(thread_id as ThreadId, mutex_ref.id());
    }

    0 // Success
}

/// Get the current thread ID.
///
/// # Returns
/// A unique identifier for the current thread, to be used with lock/unlock functions
///
/// # Safety
/// This function is `unsafe` only because it’s exposed as part of the FFI boundary, but it effectively performs a safe
/// Rust operation (getting the current thread’s ID). Callers must still ensure that this is only used within the context
/// of the same process/threading environment, and that the returned ID is used in the manner the rest of the library expects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_thread_id() -> c_ulong {
    let id = std::thread::current().id();
    let id_ptr = &id as *const _ as usize;
    id_ptr as c_ulong
}

/// Showcase the log data by sending it to the showcase server.
///
/// # Arguments
/// * `log_path` - Path to the log file as a null-terminated C string.
///
/// # Returns
/// * `0` on success
/// * `-1` if the log path is NULL or invalid UTF-8
/// * `-2` if showcasing failed (for example, file read or network error)
///
/// # Safety
/// This function dereferences `log_path`. The caller must ensure it is a valid, null-terminated
/// UTF-8 string and that the memory remains valid during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_showcase(log_path: *const c_char) -> c_int {
    if log_path.is_null() {
        return -1;
    }

    // Convert C string to Rust string.
    let path_str = unsafe {
         match CStr::from_ptr(log_path).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    // Call the Rust showcase function (which should be imported from your module).
    match crate::showcase(path_str) {
        Ok(_) => 0,    // Success
        Err(_e) => -2, // Showcase failed
    }
}