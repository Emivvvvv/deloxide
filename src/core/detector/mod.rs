pub mod mutex;
pub mod rwlock;
mod stress;
pub mod thread;
mod utils;

#[cfg(feature = "stress-test")]
use crate::core::StressConfig;
#[cfg(feature = "stress-test")]
use crate::core::StressMode;
use crate::core::graph::WaitForGraph;
use crate::core::logger::EventLogger;

use crate::core::types::{DeadlockInfo, LockId, ThreadId};
use anyhow::Result;
use crossbeam_channel::{Sender, unbounded};
use fxhash::{FxHashMap, FxHashSet};
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};

// Global dispatcher for asynchronous deadlock callback execution
// Ensures callbacks can execute even when the detecting thread is deadlocked.
lazy_static::lazy_static! {
    static ref DISPATCHER: Dispatcher = {
        Dispatcher::new()
    };
}

/// Global storage for the deadlock callback function
/// Stores the user-provided callback as `Arc<dyn Fn>` for thread-safe access.
static CALLBACK: OnceLock<Arc<dyn Fn(DeadlockInfo) + Send + Sync>> = OnceLock::new();

/// Background dispatcher for asynchronous callback execution
///
/// Runs a dedicated thread that receives deadlock events through a channel
/// and executes the registered callback. This prevents deadlocks from
/// blocking callback execution.
struct Dispatcher {
    /// Channel sender for transmitting deadlock events
    sender: Sender<DeadlockInfo>,
    /// Background thread handle
    _thread_handle: std::thread::JoinHandle<()>,
}

impl Dispatcher {
    /// Create a new dispatcher with a background thread and channel
    fn new() -> Self {
        let (tx, rx) = unbounded::<DeadlockInfo>();

        // Background thread listens for events and executes callbacks
        let thread_handle = std::thread::spawn(move || {
            while let Ok(info) = rx.recv() {
                if let Some(cb) = CALLBACK.get() {
                    cb(info);
                }
            }
        });

        Dispatcher {
            sender: tx,
            _thread_handle: thread_handle,
        }
    }

    /// Send deadlock info to background thread for callback execution
    fn send(&self, info: DeadlockInfo) {
        // Non-blocking send; events dropped if channel is full/closed
        let _ = self.sender.send(info);
    }
}

/// Main deadlock detector that maintains thread-lock relationships
///
/// The Detector is the heart of Deloxide. It tracks which threads own which locks,
/// which threads are waiting for which locks, and uses this information to detect
/// potential deadlock cycles.
///
/// # How it works
///
/// 1. The detector maintains a directed graph of threads waiting for other threads
/// 2. When a thread attempts to acquire a lock owned by another thread, an edge is added
/// 3. When a lock is acquired or released, the graph is updated
/// 4. Cycle detection is performed to identify potential deadlocks
/// 5. When a cycle is detected, the deadlock callback is invoked
pub struct Detector {
    /// Graph representing which threads are waiting for which other threads
    wait_for_graph: WaitForGraph,
    /// Maps threads to the locks they're attempting to acquire
    thread_waits_for: FxHashMap<ThreadId, LockId>,
    /// Tracks, for each thread, which locks it currently holds
    thread_holds: FxHashMap<ThreadId, FxHashSet<LockId>>,
    /// Maps Mutexes to the threads that currently own them
    mutex_owners: FxHashMap<LockId, ThreadId>,
    /// Maps RwLock IDs to the set of readers (shared lock holders)
    rwlock_readers: FxHashMap<LockId, FxHashSet<ThreadId>>,
    /// Maps RwLock IDs to the current writer (if any)
    rwlock_writer: FxHashMap<LockId, ThreadId>,
    /// Event logger for recording lock, thread operations, and interactions (logging is optional)
    logger: Option<EventLogger>,
    #[cfg(feature = "stress-test")]
    /// Stress testing mode
    stress_mode: StressMode,
    #[cfg(feature = "stress-test")]
    /// Stress testing configuration
    stress_config: Option<StressConfig>,
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector {
    /// Create a new deadlock detector
    pub fn new() -> Self {
        Detector {
            wait_for_graph: WaitForGraph::new(),
            thread_waits_for: FxHashMap::default(),
            thread_holds: FxHashMap::default(),
            mutex_owners: FxHashMap::default(),
            rwlock_readers: FxHashMap::default(),
            rwlock_writer: FxHashMap::default(),
            logger: None,
            #[cfg(feature = "stress-test")]
            stress_mode: StressMode::None,
            #[cfg(feature = "stress-test")]
            stress_config: None,
        }
    }

    #[cfg(feature = "stress-test")]
    #[allow(dead_code)]
    /// Create a new deadlock detector with stress testing config
    pub fn new_with_stress(mode: StressMode, config: Option<StressConfig>) -> Self {
        Detector {
            wait_for_graph: WaitForGraph::new(),
            thread_waits_for: FxHashMap::default(),
            thread_holds: FxHashMap::default(),
            mutex_owners: FxHashMap::default(),
            rwlock_readers: Default::default(),
            rwlock_writer: Default::default(),
            logger: None,
            stress_mode: mode,
            stress_config: config,
        }
    }

    /// Set EventLogger for logging thread, lock, and interaction events
    ///
    /// The logger records events such as:
    /// - Thread creation and exit
    /// - Lock creation and destruction
    /// - Thread-lock interactions (attempt, acquire, release)
    ///
    /// # Arguments
    /// * `logger` - An optional EventLogger instance. Pass `None` to disable logging
    pub fn set_logger(&mut self, logger: Option<EventLogger>) {
        self.logger = logger;
    }

    /// Set callback to be invoked when a deadlock is detected
    ///
    /// # Arguments
    /// * `callback` - Function to call when a deadlock is detected
    pub fn set_deadlock_callback<F>(&mut self, callback: F)
    where
        F: Fn(DeadlockInfo) + Send + Sync + 'static,
    {
        let cb: Arc<dyn Fn(DeadlockInfo) + Send + Sync> = Arc::new(callback);
        CALLBACK.set(cb).ok();
    }

    /// Flush all pending log entries to disk (method version)
    ///
    /// This method forces the associated logger (if enabled) to write all
    /// buffered events to disk immediately. This is the instance method that
    /// works on a specific detector instance.
    ///
    /// # Returns
    /// `Ok(())` if the flush succeeded or no logger is configured
    /// `Err` if the flush operation failed
    pub fn flush_logs(&self) -> Result<()> {
        if let Some(logger) = &self.logger {
            return logger.flush();
        }

        Ok(())
    }
}

// Global detector instance and logging info for ffi
lazy_static::lazy_static! {
    static ref GLOBAL_DETECTOR: Mutex<Detector> = Mutex::new(Detector::new());
    static ref IS_LOGGING_ENABLED: OnceLock<bool> = OnceLock::new();
}

/// Initialize the global detector with a deadlock callback and logger
///
/// This function sets up the global deadlock detector with a callback function
/// that will be invoked when a deadlock is detected, and optionally enables logging
/// for tracking thread and lock interactions.
///
/// # Arguments
/// * `callback` - Function to call when a deadlock is detected
/// * `logger` - Optional EventLogger for recording thread and lock events
#[allow(dead_code)]
pub fn init_detector<F>(callback: F, logger: Option<EventLogger>)
where
    F: Fn(DeadlockInfo) + Send + Sync + 'static,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.set_logger(logger);
    detector.set_deadlock_callback(callback);
}

/// Initialize the global detector with stress testing configuration and logger
///
/// This function sets up the global deadlock detector with a callback function,
/// stress testing capabilities, and optional logging.
///
/// # Arguments
/// * `callback` - Function to call when a deadlock is detected
/// * `stress_mode` - The stress testing mode to use
/// * `stress_config` - Optional stress testing configuration
/// * `logger` - Optional EventLogger for recording thread and lock events
#[cfg(feature = "stress-test")]
pub fn init_detector_with_stress<F>(
    callback: F,
    stress_mode: StressMode,
    stress_config: Option<StressConfig>,
    logger: Option<EventLogger>,
) where
    F: Fn(DeadlockInfo) + Send + Sync + 'static,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.set_logger(logger);
    detector.set_deadlock_callback(callback);
    detector.stress_mode = stress_mode;
    detector.stress_config = stress_config;
}

/// Flush all pending log entries from the global detector to disk
///
/// This function accesses the global detector instance and attempts to
/// flush its logger. Unlike the method version, this requires first
/// acquiring the global detector lock.
///
/// # Returns
/// `Ok(())` if the flush succeeded
///
/// # Errors
/// Returns an error if:
/// - The global detector lock cannot be acquired
/// - The logger flush operation fails
pub fn flush_global_detector_logs() -> Result<()> {
    let detector = GLOBAL_DETECTOR.lock();
    detector.flush_logs()
}
