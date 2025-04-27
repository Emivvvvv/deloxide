use crate::core::graph::WaitForGraph;
use crate::core::logger;
use crate::core::types::{DeadlockInfo, Events, LockId, ThreadId};
use crate::core::utils::get_current_thread_id;
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
    /// Tracks, for each thread, which locks it currently holds
    thread_holds: HashMap<ThreadId, HashSet<LockId>>,
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
            thread_holds: HashMap::new(),
        }
    }

    /// Set callback to be invoked when a deadlock is detected
    pub fn set_deadlock_callback<F>(&mut self, callback: F)
    where
        F: Fn(DeadlockInfo) + Send + 'static,
    {
        self.on_deadlock = Some(Box::new(callback));
    }

    /// Register a thread spawn
    pub fn on_thread_spawn(&mut self, thread_id: ThreadId, parent_id: Option<ThreadId>) {
        if logger::is_logging_enabled() {
            logger::log_thread_event(thread_id, parent_id, Events::Spawn);
        }
        // Ensure node exists in the wait-for graph
        self.wait_for_graph.edges.entry(thread_id).or_default();
    }

    /// Register a thread exit
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
    pub fn on_lock_create(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if logger::is_logging_enabled() {
            logger::log_lock_event(lock_id, Some(creator), Events::Spawn);
        }
    }

    /// Register a lock destruction
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
    pub fn on_lock_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if logger::is_logging_enabled() {
            logger::log_interaction_event(thread_id, lock_id, Events::Attempt);
        }

        if let Some(&owner) = self.lock_owners.get(&lock_id) {
            // record wait-for
            self.thread_waits_for.insert(thread_id, lock_id);
            self.wait_for_graph.add_edge(thread_id, owner);

            // check for a cycle involving this thread
            if let Some(cycle) = self.wait_for_graph.detect_cycle_from(thread_id) {
                // 1) compute intersection of held-locks across the cycle
                let mut iter = cycle.iter();
                let first = *iter.next().unwrap();
                let mut intersection = self
                    .thread_holds
                    .get(&first)
                    .cloned()
                    .unwrap_or_default();

                for &t in iter {
                    if let Some(holds) = self.thread_holds.get(&t) {
                        intersection = intersection
                            .intersection(holds)
                            .copied()
                            .collect();
                    } else {
                        intersection.clear();
                        break;
                    }
                }

                // 2) only report if *no* common lock (i.e., false-alarm filter)
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
                    if let Some(cb) = &self.on_deadlock {
                        cb(info);
                    }
                }
            }
        }
    }

    /// Register successful lock acquisition by a thread
    pub fn on_lock_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if logger::is_logging_enabled() {
            logger::log_interaction_event(thread_id, lock_id, Events::Acquired);
        }
        // update ownership
        self.lock_owners.insert(lock_id, thread_id);
        self.thread_waits_for.remove(&thread_id);
        // clear any wait-for edges for this thread
        self.wait_for_graph.remove_thread(thread_id);
        // record held lock
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);
    }

    /// Register lock release by a thread
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
pub fn on_lock_destroy(lock_id: LockId) {
    if let Ok(mut detector) = GLOBAL_DETECTOR.lock() {
        detector.on_lock_destroy(lock_id);
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
