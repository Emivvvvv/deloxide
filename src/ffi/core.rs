use crate::core::detector;
#[cfg(feature = "logging-and-visualization")]
use crate::core::logger;
use crate::ffi::{DEADLOCK_CALLBACK, DEADLOCK_DETECTED, INITIALIZED, IS_LOGGING_ENABLED};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::atomic::Ordering;

#[cfg(feature = "stress-test")]
use crate::StressMode;
#[cfg(feature = "stress-test")]
use crate::ffi::{STRESS_CONFIG, STRESS_MODE};

/// Initialize deloxide.
///
/// This function initializes the deadlock detector with optional logging and
/// callback functionality. It must be called before any other Deloxide functions.
///
/// # Arguments
/// * `log_path` - Path to a log file as a null-terminated C string, or NULL to disable logging.
/// * `callback` - Function pointer to call when a deadlock is detected, or NULL for no callback.
///
/// # Returns
/// * `0` on success
/// * `1` if the detector is already initialized
/// * `-1` if the log path contains invalid UTF-8
/// * `-2` if the logger failed to initialize
/// * `-3` if logging was requested but the `logging` feature is disabled
///
/// # Safety
/// This function dereferences raw pointers (`log_path`) and writes to mutable global statics:
///  - The caller must ensure `log_path` is either `NULL` or a valid null-terminated string.
///  - Concurrency must be managed so that global statics (`DEADLOCK_DETECTED` and `DEADLOCK_CALLBACK`) are
///    not mutated unsafely from multiple threads.
///  - Because this is an FFI boundary, the Rust side cannot guarantee the validity of incoming data. Callers
///    must uphold these invariants.
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

        #[cfg(feature = "logging-and-visualization")]
        let logger = if let Some(log_path) = log_path_option {
            match logger::EventLogger::with_file(log_path) {
                Ok(logger) => {
                    IS_LOGGING_ENABLED.store(true, Ordering::SeqCst);
                    Some(logger)
                }
                Err(_) => return -2, // Failed to initialize logger
            }
        } else {
            IS_LOGGING_ENABLED.store(false, Ordering::SeqCst);
            None
        };

        #[cfg(not(feature = "logging-and-visualization"))]
        {
            if log_path_option.is_some() {
                return -3; // Logging requires the logging feature
            }
            IS_LOGGING_ENABLED.store(false, Ordering::SeqCst);
        }

        // Create callback closure that sets flag and calls C callback
        let deadlock_callback = move |deadlock_info| {
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
        };

        // Create configuration object
        let config = detector::DetectorConfig {
            callback: Box::new(deadlock_callback),
            #[cfg(feature = "lock-order-graph")]
            check_lock_order: false, // FFI doesn't support lock order checking
            #[cfg(feature = "stress-test")]
            stress_mode: {
                #[cfg(feature = "stress-test")]
                {
                    match STRESS_MODE.load(Ordering::SeqCst) {
                        1 => StressMode::RandomPreemption,
                        2 => StressMode::ComponentBased,
                        _ => StressMode::None,
                    }
                }
                #[cfg(not(feature = "stress-test"))]
                {
                    // This branch shouldn't be reachable if stress-test is disabled,
                    // but we need to provide a value if the struct field exists.
                    // However, the struct field only exists if feature is enabled.
                    // So this block is only needed if feature is enabled.
                    unreachable!()
                }
            },
            #[cfg(feature = "stress-test")]
            stress_config: {
                #[cfg(feature = "stress-test")]
                {
                    #[allow(static_mut_refs)]
                    STRESS_CONFIG.take()
                }
                #[cfg(not(feature = "stress-test"))]
                {
                    None
                }
            },
            #[cfg(feature = "logging-and-visualization")]
            logger,
        };

        // Initialize detector
        detector::init_detector(config);

        INITIALIZED.store(true, Ordering::SeqCst);
        0 // Success
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
/// This function reads from mutable global static (`DEADLOCK_DETECTED`).
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
/// This function writes to mutable global static (`DEADLOCK_DETECTED`).
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
/// This function reads from a global atomic boolean.
/// This is safe to call from FFI contexts as atomics provide thread safety.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_is_logging_enabled() -> c_int {
    if IS_LOGGING_ENABLED.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}
