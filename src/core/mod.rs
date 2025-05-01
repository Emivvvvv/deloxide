//! Core module for Deloxide deadlock detection
//!
//! This module contains the central implementation of the deadlock detection
//! algorithm, tracked synchronization primitives, and supporting infrastructure.
//! It defines the main Deloxide configuration builder, types for representing
//! deadlock information, and the interfaces for tracking thread-lock relationships.

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

#[cfg(feature = "stress-test")]
pub mod stress;
#[cfg(feature = "stress-test")]
pub use stress::{StressConfig, StressMode};

use anyhow::{Context, Result};

/// Deloxide configuration builder struct
///
/// This struct provides a fluent builder API for configuring and initializing
/// the Deloxide deadlock detector.
///
/// # Example
///
/// ```no_run
/// use deloxide::{showcase_this, Deloxide};
///
/// // Initialize with default settings
/// Deloxide::new().start().expect("Failed to initialize detector");
///
/// // Initialize with logging and a custom callback
/// Deloxide::new()
///     .with_log("deadlock_logs.json")
///     .callback(|info| {
///         showcase_this().expect("Failed to launch visualization");
///         eprintln!("Deadlock detected! Threads: {:?}", info.thread_cycle);
///     })
///     .start()
///     .expect("Failed to initialize detector");
/// ```
pub struct Deloxide {
    /// Path to store log file, or None to disable logging
    log_path: Option<String>,

    /// Callback function to invoke when a deadlock is detected
    callback: Box<dyn Fn(DeadlockInfo) + Send + Sync + 'static>,

    /// Stress testing mode (only available with "stress-test" feature)
    #[cfg(feature = "stress-test")]
    stress_mode: StressMode,

    /// Stress testing configuration (only available with "stress-test" feature)
    #[cfg(feature = "stress-test")]
    stress_config: Option<StressConfig>,
}

impl Default for Deloxide {
    fn default() -> Self {
        Self::new()
    }
}

impl Deloxide {
    /// Create a new Deloxide configuration with default settings
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
            #[cfg(feature = "stress-test")]
            stress_mode: StressMode::None,
            #[cfg(feature = "stress-test")]
            stress_config: None,
        }
    }

    /// Enable logging and set the path for the log file
    ///
    /// This function enables logging of all mutex operations and thread events
    /// to a file at the specified path. This log can later be visualized using
    /// the `showcase` function.
    ///
    /// # Arguments
    /// * `path` - Path to the log file. If the path contains "{timestamp}",
    ///   it will be replaced with the current timestamp.
    ///
    /// # Returns
    /// The builder for method chaining
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Deloxide;
    ///
    /// let config = Deloxide::new()
    ///     .with_log("logs/deadlock_{timestamp}.json");
    /// ```
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Deloxide, DeadlockInfo};
    ///
    /// let config = Deloxide::new()
    ///     .callback(|info: DeadlockInfo| {
    ///         eprintln!("Deadlock detected! Thread cycle: {:?}", info.thread_cycle);
    ///         // Take remedial action, log to external system, etc.
    ///     });
    /// ```
    pub fn callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(DeadlockInfo) + Send + Sync + 'static,
    {
        self.callback = Box::new(callback);
        self
    }

    /// Initialize the deloxide deadlock detector with the configured settings
    ///
    /// This finalizes the configuration and starts the deadlock detector.
    /// After calling this method, the detector will begin monitoring lock
    /// operations and can detect deadlocks.
    ///
    /// # Returns
    /// A Result that is Ok if initialization succeeded, or an error if it failed
    ///
    /// # Errors
    /// Returns an error if logger initialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use deloxide::Deloxide;
    ///
    /// Deloxide::new()
    ///     .with_log("deadlock_log.json")
    ///     .callback(|info| {
    ///         println!("Deadlock detected: {:?}", info);
    ///     })
    ///     .start()
    ///     .expect("Failed to initialize deadlock detector");
    /// ```
    pub fn start(self) -> Result<()> {
        // Initialize the logger if a path was provided
        if let Some(log_path) = self.log_path {
            init_logger(Some(log_path)).context("Failed to initialize logger")?;
        }

        // Initialize the detector
        #[cfg(not(feature = "stress-test"))]
        {
            init_detector(self.callback);
        }

        #[cfg(feature = "stress-test")]
        {
            // Initialize detector with stress settings
            detector::init_detector_with_stress(
                self.callback,
                self.stress_mode,
                self.stress_config,
            );
        }

        // Print header
        println!("{}", crate::BANNER);

        Ok(())
    }

    /// Enable random preemption stress testing
    ///
    /// This method enables stress testing with random thread preemptions
    /// before lock acquisitions to increase deadlock probability.
    ///
    /// # Returns
    /// The builder for method chaining
    ///
    /// # Note
    /// This method is only available when the "stress-test" feature is enabled.
    #[cfg(feature = "stress-test")]
    pub fn with_random_stress(mut self) -> Self {
        self.stress_mode = StressMode::RandomPreemption;
        if self.stress_config.is_none() {
            self.stress_config = Some(StressConfig::default());
        }
        self
    }

    /// Enable component-based stress testing
    ///
    /// This method enables stress testing with strategic delays based on
    /// lock acquisition patterns to increase deadlock probability.
    ///
    /// # Returns
    /// The builder for method chaining
    ///
    /// # Note
    /// This method is only available when the "stress-test" feature is enabled.
    #[cfg(feature = "stress-test")]
    pub fn with_component_stress(mut self) -> Self {
        self.stress_mode = StressMode::ComponentBased;
        if self.stress_config.is_none() {
            self.stress_config = Some(StressConfig::default());
        }
        self
    }

    /// Configure stress testing parameters
    ///
    /// # Arguments
    /// * `config` - Configuration for stress testing
    ///
    /// # Returns
    /// The builder for method chaining
    ///
    /// # Note
    /// This method is only available when the "stress-test" feature is enabled.
    #[cfg(feature = "stress-test")]
    pub fn with_stress_config(mut self, config: StressConfig) -> Self {
        self.stress_config = Some(config);
        self
    }
}
