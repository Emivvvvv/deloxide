// Core types
pub mod types;
pub use types::*;

// Logging functionality
pub mod logger;
pub use logger::init_logger;

// Graph implementation
pub mod graph;

// Deadlock detector
pub mod detector;
#[allow(unused_imports)]
pub use detector::{
    init_detector, on_lock_acquired, on_lock_attempt, on_lock_create, on_lock_release,
    on_thread_exit, on_thread_spawn,
};

// Tracked mutex
pub mod tracked_mutex;
pub use tracked_mutex::TrackedMutex;

pub mod tracked_thread;
pub use tracked_thread::TrackedThread;

pub mod utils;

use anyhow::{Context, Result};

/// Deloxide configuration struct
pub struct Deloxide {
    log_path: Option<String>,
    callback: Box<dyn Fn(DeadlockInfo) + Send + 'static>,
}
impl Default for Deloxide {
    fn default() -> Self {
        Self::new()
    }
}

impl Deloxide {
    /// Create a new Deloxide with default settings
    ///
    /// By default:
    /// - Logging is disabled
    /// - Callback is set to panic with deadlock information
    pub fn new() -> Self {
        Deloxide {
            log_path: None,
            callback: Box::new(|info: DeadlockInfo| {
                panic!(
                    "Deadlock detected: {}",
                    serde_json::to_string_pretty(&info).unwrap_or_else(|_| format!("{:?}", info))
                );
            }),
        }
    }

    /// Activate logger and set the path for the log file
    ///
    /// # Arguments
    /// * `path` - Path to the log file. If the path contains "{timestamp}",
    ///   it will be replaced with the current timestamp.
    ///
    /// # Returns
    /// The builder for method chaining
    pub fn with_log<P: AsRef<std::path::Path>>(mut self, path: P) -> Self {
        self.log_path = Some(path.as_ref().to_string_lossy().into_owned());
        self
    }

    /// Set a custom callback to be invoked when a deadlock is detected
    ///
    /// # Arguments
    /// * `callback` - Function to call when a deadlock is detected
    ///
    /// # Returns
    /// The builder for method chaining
    pub fn callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(DeadlockInfo) + Send + 'static,
    {
        self.callback = Box::new(callback);
        self
    }

    /// Initialize the deloxide deadlock detector with the configured settings
    ///
    /// # Returns
    /// A Result that is Ok if initialization succeeded, or an error if it failed
    ///
    /// # Errors
    /// Returns an error if logger initialization fails
    pub fn start(self) -> Result<()> {
        // Initialize the logger if a path was provided
        if let Some(log_path) = self.log_path {
            init_logger(Some(log_path)).context("Failed to initialize logger")?;
        }

        // Initialize the detector with the callback
        init_detector(self.callback);

        // Print header
        println!("{}", crate::BANNER);

        Ok(())
    }
}
