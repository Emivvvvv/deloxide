//! Foreign Function Interface (FFI) for using the deloxide from C and C++ code.
//!
//! All function are unsafe since they're exposed via FFI.

use crate::core::{ThreadId, TrackedMutex, init_detector, on_lock_attempt, on_lock_acquired, on_lock_release};
use crate::core::logger;
use std::ffi::{c_void, CStr, CString};
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
/// * `log_path` - Path to log file as a null-terminated C string, or NULL to disable logging entirely
/// * `callback` - Function pointer to call when a deadlock is detected, or NULL for no callback
///
/// # Returns
/// * `0` on success
/// * `1` if detector is already initialized
/// * `-1` if the log path contains invalid UTF-8
/// * `-2` if the logger failed to initialize
#[no_mangle]
pub unsafe extern "C" fn deloxide_init(
    log_path: *const c_char,
    callback: Option<extern "C" fn(*const c_char)>
) -> c_int {
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
            init_detector(|deadlock| {
                DEADLOCK_DETECTED.store(true, Ordering::SeqCst);

                // Call C callback if provided
                if let Some(cb) = DEADLOCK_CALLBACK {
                    // Format deadlock info as JSON
                    if let Ok(json) = serde_json::to_string(&deadlock) {
                        let c_str = CString::new(json).unwrap_or_default();
                        cb(c_str.as_ptr());
                    }
                }
            });
            INITIALIZED.store(true, Ordering::SeqCst);
            0 // Success
        },
        Err(_) => -2, // Failed to initialize logger
    }
}

/// Check if a deadlock has been detected.
///
/// # Returns
/// * `1` if a deadlock was detected
/// * `0` if no deadlock has been detected
#[no_mangle]
pub unsafe extern "C" fn deloxide_is_deadlock_detected() -> c_int {
    if DEADLOCK_DETECTED.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

/// Reset the deadlock detected flag.
///
/// This allows the detector to report future deadlocks after one has been handled.
#[no_mangle]
pub unsafe extern "C" fn deloxide_reset_deadlock_flag() {
    DEADLOCK_DETECTED.store(false, Ordering::SeqCst);
}

/// Check if logging is enabled.
///
/// # Returns
/// * `1` if logging is enabled
/// * `0` if logging is disabled
#[no_mangle]
pub unsafe extern "C" fn deloxide_is_logging_enabled() -> c_int {
    if logger::is_logging_enabled() {
        1
    } else {
        0
    }
}

/// Create a new tracked mutex.
///
/// # Returns
/// * Void pointer to the mutex, or NULL on allocation failure
///
/// # Safety
/// The returned pointer must be freed with deloxide_destroy_mutex.
#[no_mangle]
pub unsafe extern "C" fn deloxide_create_mutex() -> *mut c_void {
    // Create a basic mutex with an empty value
    let mutex = Box::new(TrackedMutex::new(()));
    Box::into_raw(mutex) as *mut c_void
}

/// Destroy a tracked mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with deloxide_create_mutex
///
/// # Safety
/// The mutex pointer should not be used after calling this function.
#[no_mangle]
pub unsafe extern "C" fn deloxide_destroy_mutex(mutex: *mut c_void) {
    if !mutex.is_null() {
        drop(Box::from_raw(mutex as *mut TrackedMutex<()>));
    }
}

/// Lock a tracked mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with deloxide_create_mutex
/// * `thread_id` - ID of the thread attempting to acquire the lock, typically from deloxide_get_thread_id()
///
/// # Returns
/// * `0` on successful lock acquisition
/// * `-1` if the mutex pointer is NULL
/// * `-2` if the lock operation failed (mutex is poisoned)
#[no_mangle]
pub unsafe extern "C" fn deloxide_lock(
    mutex: *mut c_void,
    thread_id: c_ulong
) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    let mutex = &*(mutex as *const TrackedMutex<()>);

    // Register lock attempt with the detector directly
    on_lock_attempt(thread_id as ThreadId, mutex.id());

    // Try to acquire the lock
    match mutex.lock() {
        Ok(_) => {
            on_lock_acquired(thread_id as ThreadId, mutex.id());
            0 // Success
        },
        Err(_) => {
            -2 // Failed to acquire lock
        }
    }
}

/// Unlock a tracked mutex.
///
/// # Arguments
/// * `mutex` - Pointer to a mutex created with deloxide_create_mutex
/// * `thread_id` - ID of the thread releasing the lock, typically from deloxide_get_thread_id()
///
/// # Returns
/// * `0` on success
/// * `-1` if the mutex pointer is NULL
#[no_mangle]
pub unsafe extern "C" fn deloxide_unlock(
    mutex: *mut c_void,
    thread_id: c_ulong
) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    let mutex = &*(mutex as *const TrackedMutex<()>);

    // Register lock release with the detector
    on_lock_release(thread_id as ThreadId, mutex.id());

    0 // Success
}

/// Get the current thread ID.
///
/// # Returns
/// A unique identifier for the current thread, to be used with lock/unlock functions
#[no_mangle]
pub unsafe extern "C" fn deloxide_get_thread_id() -> c_ulong {
    // Use the thread ID pointer as a unique identifier
    let id = std::thread::current().id();
    let id_ptr = &id as *const _ as usize;
    id_ptr as c_ulong
}