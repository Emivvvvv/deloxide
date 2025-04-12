// New file: graph_logger.rs
use crate::core::types::{LockEvent, LockId, ThreadId};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use serde::Serialize;

/// Represents a link between a thread and a lock
#[derive(Debug, Serialize)]
pub struct GraphLink {
    /// Thread ID (source)
    pub source: ThreadId,
    /// Lock ID (target)
    pub target: LockId,
    /// Type of relationship (acquired or attempt)
    #[serde(rename = "type")]
    pub link_type: String,
}

/// Represents the complete state of the thread-lock graph
#[derive(Debug, Serialize)]
pub struct GraphState {
    /// All thread IDs in the system
    pub threads: Vec<ThreadId>,
    /// All lock IDs in the system
    pub locks: Vec<LockId>,
    /// Links between threads and locks
    pub links: Vec<GraphLink>,
}

/// Maintains the current state of threads and locks in the system
pub struct GraphLogger {
    /// Maps locks to the threads that currently own them
    lock_owners: HashMap<LockId, ThreadId>,
    /// Maps threads to the locks they're attempting to acquire
    thread_attempts: HashMap<ThreadId, HashSet<LockId>>,
    /// Set of all threads that have been seen
    threads: HashSet<ThreadId>,
    /// Set of all locks that have been seen
    locks: HashSet<LockId>,
}

impl Default for GraphLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphLogger {
    /// Create a new graph logger
    pub fn new() -> Self {
        GraphLogger {
            lock_owners: HashMap::new(),
            thread_attempts: HashMap::new(),
            threads: HashSet::new(),
            locks: HashSet::new(),
        }
    }

    /// Update the graph state based on a lock event
    pub fn update(&mut self, thread_id: ThreadId, lock_id: LockId, event: LockEvent) {
        // Track all seen threads and locks
        self.threads.insert(thread_id);
        self.locks.insert(lock_id);

        match event {
            LockEvent::Attempt => {
                // Add this attempt to the thread's attempts set
                self.thread_attempts
                    .entry(thread_id)
                    .or_insert_with(HashSet::new)
                    .insert(lock_id);
            }
            LockEvent::Acquired => {
                // Record ownership of the lock
                self.lock_owners.insert(lock_id, thread_id);

                // Remove from attempts since it's now acquired
                if let Some(attempts) = self.thread_attempts.get_mut(&thread_id) {
                    attempts.remove(&lock_id);
                }
            }
            LockEvent::Released => {
                // Remove ownership only if this thread owns it
                if self.lock_owners.get(&lock_id) == Some(&thread_id) {
                    self.lock_owners.remove(&lock_id);
                }
            }
        }
    }

    /// Generate the current graph state
    pub fn get_current_state(&self) -> GraphState {
        let mut links = Vec::new();

        // Add links for acquired locks
        for (&lock_id, &thread_id) in &self.lock_owners {
            links.push(GraphLink {
                source: thread_id,
                target: lock_id,
                link_type: "Acquired".to_string(),
            });
        }

        // Add links for attempted locks
        for (&thread_id, attempts) in &self.thread_attempts {
            for &lock_id in attempts {
                links.push(GraphLink {
                    source: thread_id,
                    target: lock_id,
                    link_type: "Attempt".to_string(),
                });
            }
        }

        GraphState {
            threads: self.threads.iter().copied().collect(),
            locks: self.locks.iter().copied().collect(),
            links,
        }
    }
}

// Global graph logger instance
lazy_static::lazy_static! {
    static ref GLOBAL_GRAPH_LOGGER: Mutex<GraphLogger> = Mutex::new(GraphLogger::new());
}

/// Update the global graph logger with a new event
pub fn update_graph(thread_id: ThreadId, lock_id: LockId, event: LockEvent) {
    if let Ok(mut logger) = GLOBAL_GRAPH_LOGGER.lock() {
        logger.update(thread_id, lock_id, event);
    }
}

/// Get the current graph state from the global logger
pub fn get_current_graph_state() -> GraphState {
    if let Ok(logger) = GLOBAL_GRAPH_LOGGER.lock() {
        logger.get_current_state()
    } else {
        // Return empty state if can't acquire lock
        GraphState {
            threads: Vec::new(),
            locks: Vec::new(),
            links: Vec::new(),
        }
    }
}