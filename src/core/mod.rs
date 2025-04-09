// Core types
pub mod types;
pub use types::*;

// Logging functionality
pub mod logger;
pub use logger::{init_logger, log_event};

// Graph implementation
pub mod graph;

// Deadlock detector
pub mod detector;
pub use detector::{init_detector, on_lock_acquired, on_lock_attempt, on_lock_release};

// Tracked mutex
pub mod tracked_mutex;
pub use tracked_mutex::TrackedMutex;

/// Initialize the deloxide with default settings
///
/// # Arguments
/// * `log_path` - Optional path to the log file (None to disable logging)
/// * `on_deadlock` - Callback to invoke when a deadlock is detected
pub fn init<P, F>(log_path: Option<P>, on_deadlock: F) -> std::io::Result<()>
where
    P: AsRef<std::path::Path>,
    F: Fn(DeadlockInfo) + Send + 'static,
{
    // Initialize the logger
    init_logger(log_path)?;

    // Initialize the detector
    init_detector(on_deadlock);

    Ok(())
}
