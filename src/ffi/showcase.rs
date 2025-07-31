use crate::core::detector;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

/// Flush all pending log entries to disk
///
/// This function forces all buffered log entries to be written to disk.
/// It should be called before reading or processing the log file to ensure completeness.
///
/// # Returns
/// * `0` on success
/// * `-1` if flushing failed
///
/// # Safety
/// This function accesses global state and should be called from a single thread at a time.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_flush_logs() -> c_int {
    match detector::flush_global_detector_logs() {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Failed to flush logs: {e:#}");
            -1
        }
    }
}

/// Showcase the log data by sending it to the showcase server.
///
/// This function flushes pending log entries, processes a log file, and opens
/// a web browser to visualize the thread-lock relationships recorded in the log.
///
/// # Arguments
/// * `log_path` - Path to the log file as a null-terminated C string.
///
/// # Returns
/// * `0` on success
/// * `-1` if the log path is NULL or invalid UTF-8
/// * `-2` if showcasing failed (for example, file read or network error)
/// * `-3` if flushing failed
///
/// # Safety
/// This function dereferences `log_path`. The caller must ensure it is a valid, null-terminated
/// UTF-8 string and that the memory remains valid during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_showcase(log_path: *const c_char) -> c_int {
    if log_path.is_null() {
        return -1;
    }

    // Convert C string to Rust string
    let path_str = unsafe {
        match CStr::from_ptr(log_path).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    // First flush all logs
    if let Err(e) = detector::flush_global_detector_logs() {
        eprintln!("Failed to flush logs: {e:#}");
        return -3;
    }

    // Call the Rust showcase function
    match crate::showcase(path_str) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Showcase error: {e:#}");
            -2
        }
    }
}

/// Showcase the current active log data by sending it to the showcase server.
///
/// This function ensures all buffered log entries are flushed to disk before showcasing
/// the log file that was specified in the deloxide_init() call.
///
/// # Returns
/// * `0` on success
/// * `-1` if no active log file exists
/// * `-2` if showcasing failed (for example, file read or network error)
/// * `-3` if flushing failed
///
/// # Safety
/// This function accesses global state and should be called from a single thread at a time.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_showcase_current() -> c_int {
    // First flush all logs
    if let Err(e) = detector::flush_global_detector_logs() {
        eprintln!("Failed to flush logs: {e:#}");
        return -3;
    }

    match crate::showcase::showcase_this() {
        Ok(_) => 0,
        Err(e) => {
            if e.to_string().contains("No active log file") {
                -1
            } else {
                eprintln!("Showcase error: {e:#}");
                -2
            }
        }
    }
}
