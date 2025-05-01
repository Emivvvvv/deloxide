#[cfg(feature = "stress-test")]
use crate::core::StressConfig;
#[cfg(feature = "stress-test")]
use crate::core::StressMode;
use crate::core::graph::WaitForGraph;
use crate::core::logger;
#[cfg(feature = "stress-test")]
use crate::core::stress::{
    on_lock_attempt as stress_on_lock_attempt, on_lock_release as stress_on_lock_release,
};
use crate::core::types::{DeadlockInfo, Events, LockId, ThreadId, get_current_thread_id};
use chrono::Utc;
use crossbeam_channel::{Sender, unbounded};
use fxhash::{FxHashMap, FxHashSet};
use std::sync::{Arc, Mutex, OnceLock};

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
    /// Maps locks to the threads that currently own them
    lock_owners: FxHashMap<LockId, ThreadId>,
    /// Maps threads to the locks they're attempting to acquire
    thread_waits_for: FxHashMap<ThreadId, LockId>,
    /// Tracks, for each thread, which locks it currently holds
    thread_holds: FxHashMap<ThreadId, FxHashSet<LockId>>,
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
            lock_owners: FxHashMap::default(),
            thread_waits_for: FxHashMap::default(),
            thread_holds: FxHashMap::default(),
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
            lock_owners: FxHashMap::default(),
            thread_waits_for: FxHashMap::default(),
            thread_holds: FxHashMap::default(),
            stress_mode: mode,
            stress_config: config,
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

    /// Register a thread spawn
    ///
    /// This method is called when a new thread is created. It records the thread
    /// in the wait-for graph and establishes parent-child relationships for proper
    /// resource tracking.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the newly spawned thread
    /// * `parent_id` - Optional ID of the parent thread that created this thread
    pub fn on_thread_spawn(&mut self, thread_id: ThreadId, parent_id: Option<ThreadId>) {
        if logger::is_logging_enabled() {
            logger::log_thread_event(thread_id, parent_id, Events::Spawn);
        }
        // Ensure node exists in the wait-for graph
        self.wait_for_graph.edges.entry(thread_id).or_default();
    }

    /// Register a thread exit
    ///
    /// This method is called when a thread is about to exit. It cleans up resources
    /// associated with the thread and updates the wait-for graph.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the exiting thread
    pub fn on_thread_exit(&mut self, thread_id: ThreadId) {
        if logger::is_logging_enabled() {
            logger::log_thread_event(thread_id, None, Events::Exit);
        }
        // remove thread and its edges from the wait-for graph
        self.wait_for_graph.remove_thread(thread_id);
        // no more held locks
        self.thread_holds.remove(&thread_id);
    }

    /// Register a lock creation
    ///
    /// This method is called when a new mutex is created. It records which thread
    /// created the mutex for proper resource tracking.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created lock
    /// * `creator_id` - Optional ID of the thread that created this lock
    pub fn on_lock_create(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if logger::is_logging_enabled() {
            logger::log_lock_event(lock_id, Some(creator), Events::Spawn);
        }
    }

    /// Register a lock destruction
    ///
    /// This method is called when a mutex is being destroyed. It cleans up
    /// all references to the lock in the detector's data structures.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the lock being destroyed
    pub fn on_lock_destroy(&mut self, lock_id: LockId) {
        // remove ownership
        self.lock_owners.remove(&lock_id);
        // clear any pending wait-for for this lock
        for attempts in self.thread_waits_for.values_mut() {
            if *attempts == lock_id {
                *attempts = 0;
            }
        }
        self.thread_waits_for.retain(|_, &mut l| l != 0);

        if logger::is_logging_enabled() {
            logger::log_lock_event(lock_id, None, Events::Exit);
        }
        // purge from all held-lock sets
        for holds in self.thread_holds.values_mut() {
            holds.remove(&lock_id);
        }
    }

    /// Register a lock attempt by a thread
    ///
    /// This method is called when a thread attempts to acquire a mutex. It records
    /// the attempt in the thread-lock relationship graph and checks for potential
    /// deadlock cycles.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the lock
    /// * `lock_id` - ID of the lock being attempted
    pub fn on_lock_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if logger::is_logging_enabled() {
            logger::log_interaction_event(thread_id, lock_id, Events::Attempt);
        }

        #[cfg(feature = "stress-test")]
        {
            if self.stress_mode != StressMode::None {
                if let Some(config) = &self.stress_config {
                    let held_locks = self
                        .thread_holds
                        .get(&thread_id)
                        .map(|set| set.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default();

                    stress_on_lock_attempt(
                        self.stress_mode,
                        thread_id,
                        lock_id,
                        &held_locks,
                        config,
                    );
                }
            }
        }

        if let Some(&owner) = self.lock_owners.get(&lock_id) {
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                // Apply filter for common locks
                let mut iter = cycle.iter();
                let first = *iter.next().unwrap();
                let mut intersection = self.thread_holds.get(&first).cloned().unwrap_or_default();

                for &t in iter {
                    if let Some(holds) = self.thread_holds.get(&t) {
                        intersection = intersection.intersection(holds).copied().collect();
                    } else {
                        intersection.clear();
                        break;
                    }
                }

                // Only report if no common lock (i.e., false-alarm filter)
                if intersection.is_empty() {
                    let info = DeadlockInfo {
                        thread_cycle: cycle.clone(),
                        thread_waiting_for_locks: self
                            .thread_waits_for
                            .iter()
                            .map(|(&t, &l)| (t, l))
                            .collect(),
                        timestamp: Utc::now().to_rfc3339(),
                    };

                    // Send to dispatcher instead of calling directly
                    DISPATCHER.send(info);
                }
            }
        }
    }

    /// Register successful lock acquisition by a thread
    ///
    /// This method is called when a thread successfully acquires a mutex. It updates
    /// the ownership information and clears any wait-for edges in the graph.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the lock
    /// * `lock_id` - ID of the lock that was acquired
    pub fn on_lock_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if logger::is_logging_enabled() {
            logger::log_interaction_event(thread_id, lock_id, Events::Acquired);
        }

        // Update ownership
        self.lock_owners.insert(lock_id, thread_id);
        self.thread_waits_for.remove(&thread_id);

        // Remove thread from wait graph
        self.wait_for_graph.remove_thread(thread_id);

        // Record held lock
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);
    }

    /// Register lock release by a thread
    ///
    /// This method is called when a thread releases a mutex. It updates the ownership
    /// information in the detector's data structures.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the lock
    /// * `lock_id` - ID of the lock being released
    pub fn on_lock_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if logger::is_logging_enabled() {
            logger::log_interaction_event(thread_id, lock_id, Events::Released);
        }
        if self.lock_owners.get(&lock_id) == Some(&thread_id) {
            self.lock_owners.remove(&lock_id);
        }
        // remove from held-locks
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        // Apply post-release stress testing if enabled
        #[cfg(feature = "stress-test")]
        {
            if self.stress_mode != StressMode::None {
                if let Some(config) = &self.stress_config {
                    stress_on_lock_release(self.stress_mode, thread_id, lock_id, config);
                }
            }
        }
    }
}

// Global detector instance
lazy_static::lazy_static! {
    static ref GLOBAL_DETECTOR: Mutex<Detector> = Mutex::new(Detector::new());
}

/// Initialize the global detector with a deadlock callback
///
/// This function sets up the global deadlock detector with a callback function
/// that will be invoked when a deadlock is detected.
///
/// # Arguments
/// * `callback` - Function to call when a deadlock is detected
#[allow(dead_code)]
pub fn init_detector<F>(callback: F)
where
    F: Fn(DeadlockInfo) + Send + Sync + 'static,
{
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.set_deadlock_callback(callback);
    }
}

#[cfg(feature = "stress-test")]
/// Initialize the global detector with stress testing configuration
pub fn init_detector_with_stress<F>(
    callback: F,
    stress_mode: StressMode,
    stress_config: Option<StressConfig>,
) where
    F: Fn(DeadlockInfo) + Send + Sync + 'static,
{
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.set_deadlock_callback(callback);
        detector.stress_mode = stress_mode;
        detector.stress_config = stress_config;
    }
}

/// Register a thread spawn with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the spawned thread
/// * `parent_id` - Optional ID of the parent thread that created this thread
pub fn on_thread_spawn(thread_id: ThreadId, parent_id: Option<ThreadId>) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_thread_spawn(thread_id, parent_id);
    }
}

/// Register a thread exit with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the exiting thread
pub fn on_thread_exit(thread_id: ThreadId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_thread_exit(thread_id);
    }
}

/// Register a lock creation with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the created lock
/// * `creator_id` - Optional ID of the thread that created this lock
pub fn on_lock_create(lock_id: LockId, creator_id: Option<ThreadId>) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_create(lock_id, creator_id);
    }
}

/// Register a lock destruction with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the lock being destroyed
pub fn on_lock_destroy(lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_destroy(lock_id);
    }
}

/// Register a lock attempt with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the lock
/// * `lock_id` - ID of the lock being attempted
pub fn on_lock_attempt(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_attempt(thread_id, lock_id);
    }
}

/// Register a lock acquisition with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the lock
/// * `lock_id` - ID of the lock that was acquired
pub fn on_lock_acquired(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_acquired(thread_id, lock_id);
    }
}

/// Register a lock release with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread releasing the lock
/// * `lock_id` - ID of the lock being released
pub fn on_lock_release(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_release(thread_id, lock_id);
    }
}
