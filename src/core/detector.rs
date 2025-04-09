use crate::core::graph::WaitForGraph;
use crate::core::types::{DeadlockInfo, LockId, ThreadId};
use crate::core::{LockEvent, logger};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Main deadlock detector that maintains thread-lock relationships
pub struct Detector {
    /// Graph representing which threads are waiting for which other threads
    wait_for_graph: WaitForGraph,

    /// Maps locks to the threads that currently own them
    lock_owners: HashMap<LockId, ThreadId>,

    /// Maps threads to the locks they're attempting to acquire
    thread_waits_for: HashMap<ThreadId, LockId>,

    /// Callback to invoke when a deadlock is detected
    on_deadlock: Option<Box<dyn Fn(DeadlockInfo) + Send>>,
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
            lock_owners: HashMap::new(),
            thread_waits_for: HashMap::new(),
            on_deadlock: None,
        }
    }

    /// Set callback to be invoked when a deadlock is detected
    pub fn set_deadlock_callback<F>(&mut self, callback: F)
    where
        F: Fn(DeadlockInfo) + Send + 'static,
    {
        self.on_deadlock = Some(Box::new(callback));
    }

    /// Register a lock attempt by a thread
    pub fn on_lock_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        // Log the attempt
        if logger::is_logging_enabled() {
            logger::log_event(thread_id, lock_id, LockEvent::Attempt);
        }

        // Check if the lock is already owned
        if let Some(&owner) = self.lock_owners.get(&lock_id) {
            // Record that this thread is waiting for this lock
            self.thread_waits_for.insert(thread_id, lock_id);

            // Update the Wait-For Graph to show this thread is waiting for the lock owner
            self.wait_for_graph.add_edge(thread_id, owner);

            // Check for deadlock
            if let Some(cycle) = self.wait_for_graph.detect_cycle_from(thread_id) {
                // Create deadlock info
                let thread_waiting_for_locks: Vec<(ThreadId, LockId)> = self
                    .thread_waits_for
                    .iter()
                    .map(|(&t_id, &l_id)| (t_id, l_id))
                    .collect();

                let deadlock_info = DeadlockInfo {
                    thread_cycle: cycle,
                    thread_waiting_for_locks,
                    timestamp: Utc::now().to_rfc3339(),
                };

                // Call the deadlock callback if set
                if let Some(callback) = &self.on_deadlock {
                    callback(deadlock_info);
                }
            }
        }
    }

    /// Register successful lock acquisition by a thread
    pub fn on_lock_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        // Log the acquisition
        if logger::is_logging_enabled() {
            logger::log_event(thread_id, lock_id, LockEvent::Acquired);
        }

        // Update state
        self.lock_owners.insert(lock_id, thread_id);
        self.thread_waits_for.remove(&thread_id);

        // This thread is no longer waiting for anything
        self.wait_for_graph.remove_thread(thread_id);
    }

    /// Register lock release by a thread
    pub fn on_lock_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
        // Log the release
        if logger::is_logging_enabled() {
            logger::log_event(thread_id, lock_id, LockEvent::Released);
        }

        // Only remove ownership if this thread actually owns the lock
        if self.lock_owners.get(&lock_id) == Some(&thread_id) {
            self.lock_owners.remove(&lock_id);
        }
    }

    /// Get a snapshot of the current wait-for graph
    pub fn get_wait_for_graph(&self) -> HashMap<ThreadId, HashSet<ThreadId>> {
        self.wait_for_graph.get_edges()
    }
}

// Global detector instance
lazy_static::lazy_static! {
    static ref GLOBAL_DETECTOR: Mutex<Detector> = Mutex::new(Detector::new());
}

/// Initialize the global detector with a deadlock callback
pub fn init_detector<F>(callback: F)
where
    F: Fn(DeadlockInfo) + Send + 'static,
{
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.set_deadlock_callback(callback);
    }
}

/// Register a lock attempt with the global detector
pub fn on_lock_attempt(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_attempt(thread_id, lock_id);
    }
}

/// Register a lock acquisition with the global detector
pub fn on_lock_acquired(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_acquired(thread_id, lock_id);
    }
}

/// Register a lock release with the global detector
pub fn on_lock_release(thread_id: ThreadId, lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_release(thread_id, lock_id);
    }
}
