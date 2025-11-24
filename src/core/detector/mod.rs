pub mod condvar;
pub mod deadlock_handling;
pub mod mutex;
pub mod rwlock;
mod stress;
pub mod thread;

#[cfg(feature = "stress-test")]
use crate::core::StressConfig;
#[cfg(feature = "stress-test")]
use crate::core::StressMode;
#[cfg(feature = "lock-order-graph")]
use crate::core::graph::LockOrderGraph;
use crate::core::graph::WaitForGraph;
#[cfg(feature = "logging-and-visualization")]
use crate::core::logger::{self, EventLogger};

use crate::core::types::{CondvarId, DeadlockInfo, LockId, ThreadId};
#[cfg(feature = "logging-and-visualization")]
use anyhow::Result;
use fxhash::{FxHashMap, FxHashSet};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};

/// Configuration for the deadlock detector
pub struct DetectorConfig {
    /// Callback function to invoke when a deadlock is detected
    pub callback: Box<dyn Fn(DeadlockInfo) + Send + Sync>,
    /// Enable lock order checking
    #[cfg(feature = "lock-order-graph")]
    pub check_lock_order: bool,
    /// Stress testing mode
    #[cfg(feature = "stress-test")]
    pub stress_mode: StressMode,
    /// Stress testing configuration
    #[cfg(feature = "stress-test")]
    pub stress_config: Option<StressConfig>,
    /// Logger for recording events
    #[cfg(feature = "logging-and-visualization")]
    pub logger: Option<EventLogger>,
}

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
        let (tx, rx) = channel::<DeadlockInfo>();

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
    /// Lock order graph for detecting lock ordering violations (only created if enabled)
    #[cfg(feature = "lock-order-graph")]
    lock_order_graph: Option<LockOrderGraph>,
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
    /// Maps condvar IDs to queues of waiting threads and their associated mutex IDs
    cv_waiters: FxHashMap<CondvarId, VecDeque<(ThreadId, LockId)>>,
    /// Maps threads to the condvar and mutex they're waiting on
    thread_wait_cv: FxHashMap<ThreadId, (CondvarId, LockId)>,
    /// Set of threads that have been woken from condvar waits (for diagnostics)
    cv_woken: FxHashSet<ThreadId>,
    /// Maps locks to the set of threads waiting for them (for stale edge removal)
    lock_waiters: FxHashMap<LockId, FxHashSet<ThreadId>>,
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
    /// Create a new deadlock detector with default settings
    ///
    /// By default, lock order checking is disabled.
    pub fn new() -> Self {
        Detector {
            wait_for_graph: WaitForGraph::new(),
            #[cfg(feature = "lock-order-graph")]
            lock_order_graph: None, // Not created by default
            thread_waits_for: FxHashMap::default(),
            thread_holds: FxHashMap::default(),
            mutex_owners: FxHashMap::default(),
            rwlock_readers: FxHashMap::default(),
            rwlock_writer: FxHashMap::default(),
            cv_waiters: FxHashMap::default(),
            thread_wait_cv: FxHashMap::default(),
            cv_woken: FxHashSet::default(),
            lock_waiters: FxHashMap::default(),
            #[cfg(feature = "stress-test")]
            stress_mode: StressMode::None,
            #[cfg(feature = "stress-test")]
            stress_config: None,
        }
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

    /// Check for lock order violations when a thread attempts to acquire a lock
    #[cfg(feature = "lock-order-graph")]
    fn check_lock_order_violation(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
    ) -> Option<Vec<LockId>> {
        // Only check if lock order graph is enabled
        let graph = self.lock_order_graph.as_mut()?;

        if let Some(held_locks) = self.thread_holds.get(&thread_id) {
            for &held_lock in held_locks {
                if let Some(lock_cycle) = graph.add_edge(held_lock, lock_id) {
                    return Some(lock_cycle);
                }
            }
        }
        None
    }
}

// Global detector instance and logging info for ffi
lazy_static::lazy_static! {
    static ref GLOBAL_DETECTOR: Mutex<Detector> = Mutex::new(Detector::new());
}

/// Initialize the global detector with the provided configuration
///
/// This function sets up the global deadlock detector with the specified
/// callback, stress testing options, and logging configuration.
///
/// # Arguments
/// * `config` - The configuration object for the detector
pub fn init_detector(config: DetectorConfig) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.set_deadlock_callback(config.callback);

    #[cfg(feature = "logging-and-visualization")]
    if let Some(logger) = config.logger {
        logger::init_logger(logger);
    }

    // Create lock order graph if enabled
    #[cfg(feature = "lock-order-graph")]
    if config.check_lock_order {
        detector.lock_order_graph = Some(LockOrderGraph::new());
    }
    #[cfg(not(feature = "lock-order-graph"))]
    #[cfg(feature = "lock-order-graph")]
    // Only warn if the field exists in config but feature is off? No, field doesn't exist.
    {} // No-op if feature is off

    #[cfg(feature = "stress-test")]
    {
        detector.stress_mode = config.stress_mode;
        detector.stress_config = config.stress_config;
    }
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
#[cfg(feature = "logging-and-visualization")]
pub fn flush_global_detector_logs() -> Result<()> {
    logger::flush_logs()
}
