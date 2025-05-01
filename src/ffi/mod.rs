/// FFI bindings for Deloxide C API
///
/// This module provides the C API bindings for the Deloxide deadlock detection library.
/// It maps C function calls to their Rust implementations, handling memory management,
/// thread tracking, and callback mechanisms to bridge the language boundary.
///
/// The FFI interface provides all the functionality needed to use Deloxide from C or C++,
/// including initialization, mutex tracking, thread tracking, and deadlock detection.
use crate::core::logger;
use crate::core::tracked_mutex::TrackedGuard;
use crate::core::types::get_current_thread_id;
use crate::core::{
    ThreadId, TrackedMutex, on_lock_create, on_thread_exit, on_thread_spawn,
};
use serde_json;
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_void, c_double};
use std::os::raw::{c_char, c_int, c_ulong};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(feature = "stress-test"))]
use crate::core::init_detector;

#[cfg(feature = "stress-test")]
use crate::core::detector;
#[cfg(feature = "stress-test")]
use crate::core::{StressMode, StressConfig};
#[cfg(feature = "stress-test")]
use std::sync::atomic::AtomicU8;
#[cfg(feature = "stress-test")]
static STRESS_MODE: AtomicU8 = AtomicU8::new(0); // 0=None, 1=Random, 2=Component
#[cfg(feature = "stress-test")]
static mut STRESS_CONFIG: Option<StressConfig> = None;

// We'll keep each Rust guard alive here until the C code calls unlock.
thread_local! {
    // Each thread can hold exactly one guard at a time.
    static FFI_GUARD: RefCell<Option<TrackedGuard<'static, ()>>> = const {RefCell::new(None)};
}

// Globals to track initialization state
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut DEADLOCK_DETECTED: AtomicBool = AtomicBool::new(false);

// Optional callback function provided by C code
static mut DEADLOCK_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

/// Initialize deloxide.
///
/// This function initializes the deadlock detector with optional logging and
/// callback functionality. It must be called before any other Deloxide functions.
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

        #[cfg(feature = "stress-test")]
        {
            // Get stress settings if feature is enabled
            let stress_mode = match STRESS_MODE.load(Ordering::SeqCst) {
                1 => StressMode::RandomPreemption,
                2 => StressMode::ComponentBased,
                _ => StressMode::None,
            };

            #[allow(static_mut_refs)]
            let stress_config = STRESS_CONFIG.take();

            // Initialize with a callback that sets a flag and calls the C callback
            match logger::init_logger(log_path_option) {
                Ok(_) => {
                    // Initialize detector with stress settings
                    detector::init_detector_with_stress(
                        |deadlock_info| {
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
                        },
                        stress_mode,
                        stress_config,
                    );

                    INITIALIZED.store(true, Ordering::SeqCst);
                    0 // Success
                }
                Err(_) => -2, // Failed to initialize logger
            }
        }

        #[cfg(not(feature = "stress-test"))]
        {
            // Standard initialization without stress testing
            match logger::init_logger(log_path_option) {
                Ok(_) => {
                    // Initialize with a callback that sets a flag and calls the C callback
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
}

/// Check if a deadlock has been detected.
///
/// This function returns whether the deadlock detector has detected a deadlock
/// since initialization or since the last call to deloxide_reset_deadlock_flag().
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
/// Call this function after processing a deadlock notification if you want to
/// continue monitoring for additional deadlocks.
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
/// This function is safe to call from FFI contexts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_is_logging_enabled() -> c_int {
    if logger::is_logging_enabled() { 1 } else { 0 }
}

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
    let mutex = Box::new(TrackedMutex::new(()));
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
    creator_thread_id: c_ulong,
) -> *mut c_void {
    let mutex = Box::new(TrackedMutex::new(()));

    // Register the specified thread as the creator
    on_lock_create(mutex.id(), Some(creator_thread_id as ThreadId));

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
            drop(Box::from_raw(mutex as *mut TrackedMutex<()>));
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
/// - The caller must pass a valid pointer to a `TrackedMutex<()>`.
/// - The lock is re-entrant in the sense of C code, but you must not call `deloxide_lock` twice on the same mutex from the same thread without calling `deloxide_unlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_lock(mutex: *mut c_void) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const TrackedMutex<()>);
        let guard = mutex_ref.lock();

        #[allow(clippy::missing_transmute_annotations)]
        FFI_GUARD.with(|slot| *slot.borrow_mut() = Some(std::mem::transmute(guard)));
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
/// - The pointer must be valid (i.e., a previously created `TrackedMutex<()>`).
/// - The mutex must have been previously locked by the current thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_unlock(mutex: *mut c_void) -> c_int {
    if mutex.is_null() {
        return -1;
    }

    // Drop the guard we stashed above; this actually unlocks the Mutex
    FFI_GUARD.with(|slot| {
        let _ = slot.borrow_mut().take();
    });

    0
}

/// Register a thread spawn with the global detector.
///
/// This function should be called when a new thread is created to enable
/// proper tracking of thread relationships and resources.
///
/// # Arguments
/// * `thread_id` - ID of the spawned thread.
/// * `parent_id` - ID of the parent thread that created this thread, or 0 for no parent.
///
/// # Returns
/// * `0` on success
///
/// # Safety
/// - The caller must ensure thread_id represents a real thread.
/// - This function is normally called automatically by the CREATE_TRACKED_THREAD macro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_register_thread_spawn(
    thread_id: c_ulong,
    parent_id: c_ulong,
) -> c_int {
    let parent = if parent_id == 0 {
        None
    } else {
        Some(parent_id as ThreadId)
    };
    on_thread_spawn(thread_id as ThreadId, parent);
    0 // Success
}

/// Register a thread exit with the global detector.
///
/// This function should be called when a thread is about to exit to ensure
/// proper cleanup of resources owned by the thread.
///
/// # Arguments
/// * `thread_id` - ID of the exiting thread.
///
/// # Returns
/// * `0` on success
///
/// # Safety
/// - The caller must ensure thread_id represents a real thread that is exiting.
/// - This function is normally called automatically by the CREATE_TRACKED_THREAD macro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_register_thread_exit(thread_id: c_ulong) -> c_int {
    on_thread_exit(thread_id as ThreadId);
    0 // Success
}

/// Get the current thread ID.
///
/// Returns a unique identifier for the current thread that can be used
/// with other Deloxide functions.
///
/// # Returns
/// A unique identifier for the current thread as an unsigned long
///
/// # Safety
/// This function is safe to call from FFI contexts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_thread_id() -> c_ulong {
    get_current_thread_id() as c_ulong
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
/// - The caller must pass a valid pointer to a `TrackedMutex<()>`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_mutex_creator(mutex: *mut c_void) -> c_ulong {
    if mutex.is_null() {
        return 0;
    }

    unsafe {
        let mutex_ref = &*(mutex as *const TrackedMutex<()>);
        mutex_ref.creator_thread_id() as c_ulong
    }
}

/// Showcase the log data by sending it to the showcase server.
///
/// This function processes a log file and opens a web browser to visualize
/// the thread-lock relationships recorded in the log.
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

    // Call the Rust showcase function handling anyhow Result
    match crate::showcase(path_str) {
        Ok(_) => 0, // Success
        Err(e) => {
            // Log the error in a way that's accessible to C code
            eprintln!("Showcase error: {:#}", e);
            -2 // Showcase failed
        }
    }
}

/// Showcase the current active log data by sending it to the showcase server.
///
/// This is a convenience function that showcases the log file that was specified
/// in the deloxide_init() call. It's useful when you don't want to keep track of
/// the log file path manually.
///
/// # Returns
/// * `0` on success
/// * `-1` if no active log file exists
/// * `-2` if showcasing failed (for example, file read or network error)
///
/// # Safety
/// This function is safe to call from FFI contexts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_showcase_current() -> c_int {
    match crate::showcase::showcase_this() {
        Ok(_) => 0, // Success
        Err(e) => {
            if e.to_string().contains("No active log file") {
                -1 // No active log file
            } else {
                // Log the error in a way that's accessible to C code
                eprintln!("Showcase error: {:#}", e);
                -2 // Showcase failed
            }
        }
    }
}

/// Enable random preemption stress testing (only with "stress-test" feature)
///
/// This function enables stress testing with random preemptions before lock
/// acquisitions to increase deadlock probability.
///
/// # Arguments
/// * `probability` - Probability of preemption (0.0-1.0)
/// * `min_delay_ms` - Minimum delay duration in milliseconds
/// * `max_delay_ms` - Maximum delay duration in milliseconds
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub unsafe extern "C" fn deloxide_enable_random_stress(
    probability: c_double,
    min_delay_ms: c_ulong,
    max_delay_ms: c_ulong
) -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(1, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = Some(StressConfig {
                preemption_probability: probability,
                min_delay_ms,
                max_delay_ms,
                preempt_after_release: true,
            });
        }

        return 0;
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        return -1;
    }
}

/// Enable component-based stress testing (only with "stress-test" feature)
///
/// This function enables stress testing with targeted delays based on lock
/// acquisition patterns to increase deadlock probability.
///
/// # Arguments
/// * `min_delay_ms` - Minimum delay duration in milliseconds
/// * `max_delay_ms` - Maximum delay duration in milliseconds
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub unsafe extern "C" fn deloxide_enable_component_stress(
    min_delay_ms: c_ulong,
    max_delay_ms: c_ulong
) -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(2, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = Some(StressConfig {
                preemption_probability: 0.8, // High probability for component-based mode
                min_delay_ms,
                max_delay_ms,
                preempt_after_release: true,
            });
        }

        return 0;
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        return -1;
    }
}

/// Disable stress testing (only with "stress-test" feature)
///
/// This function disables any previously enabled stress testing mode.
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_disable_stress() -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(0, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = None;
        }

        return 0;
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        return -1;
    }
}
