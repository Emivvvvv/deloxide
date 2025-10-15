//! Core module for Deloxide deadlock detection
//!
//! This module contains the central implementation of the deadlock detection
//! algorithm, tracked synchronization primitives, and supporting infrastructure.
//! It defines the main Deloxide configuration builder, types for representing
//! deadlock information, and the interfaces for tracking thread-lock relationships.

// Core types
pub(crate) mod types;
pub(crate) use types::*;

// Logging functionality
pub(crate) mod logger;

// Graph implementations (wait-for and lock order graphs)
pub(crate) mod graph;

// Deadlock detector
pub(crate) mod detector;
#[allow(unused_imports)]
pub(crate) use detector::*;

pub mod thread;

pub(crate) mod locks;
#[cfg(feature = "stress-test")]
pub mod stress;

#[cfg(feature = "stress-test")]
pub use stress::{StressConfig, StressMode};

use anyhow::Result;
use logger::EventLogger;

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

    /// Enable lock order checking for potential deadlock detection
    check_lock_order: bool,

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
    /// - Lock order checking is disabled (only actual deadlocks detected)
    pub fn new() -> Self {
        Deloxide {
            log_path: None,
            callback: Box::new(|info: DeadlockInfo| {
                panic!(
                    "Deadlock detected: {}",
                    serde_json::to_string_pretty(&info).unwrap_or_else(|_| format!("{info:?}"))
                );
            }),
            check_lock_order: false,
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

    /// Enable lock order checking for potential deadlock detection
    ///
    /// When enabled, the detector will check for inconsistent lock ordering patterns
    /// that could lead to deadlocks, even if no actual deadlock has occurred yet.
    /// This provides early warning of potential deadlock bugs.
    ///
    /// **Note**: This may report patterns that never actually deadlock (false positives).
    /// Recommended for development and testing, not production.
    ///
    /// # Returns
    /// The builder for method chaining
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Deloxide;
    ///
    /// // Enable lock order checking for development
    /// Deloxide::new()
    ///     .with_lock_order_checking()
    ///     .callback(|info| {
    ///         use deloxide::DeadlockSource;
    ///         match info.source {
    ///             DeadlockSource::WaitForGraph => {
    ///                 println!("🚨 ACTUAL DEADLOCK! Threads are blocked.");
    ///             }
    ///             DeadlockSource::LockOrderViolation => {
    ///                 println!("⚠️  SUSPECTED DEADLOCK! Dangerous lock ordering pattern.");
    ///             }
    ///         }
    ///     })
    ///     .start()
    ///     .expect("Failed to start detector");
    /// ```
    pub fn with_lock_order_checking(mut self) -> Self {
        self.check_lock_order = true;
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
        let logger = if let Some(log_path) = self.log_path {
            Some(EventLogger::with_file(log_path)?)
        } else {
            None
        };

        // Initialize the detector
        #[cfg(not(feature = "stress-test"))]
        {
            init_detector(self.callback, self.check_lock_order, logger);
        }

        #[cfg(feature = "stress-test")]
        {
            // Initialize detector with stress settings
            detector::init_detector_with_stress(
                self.callback,
                self.check_lock_order,
                self.stress_mode,
                self.stress_config,
                logger,
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
