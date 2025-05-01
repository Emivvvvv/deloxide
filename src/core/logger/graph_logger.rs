use crate::core::types::{Events, LockId, ThreadId};
use fxhash::{FxHashMap, FxHashSet};
use serde::Serialize;
use std::sync::Mutex;

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
    /// Maps locks to the threads that currently own them (runtime ownership)
    lock_owners: FxHashMap<LockId, ThreadId>,

    /// Maps threads to the locks they're attempting to acquire
    thread_attempts: FxHashMap<ThreadId, FxHashSet<LockId>>,

    /// Maps locks to the threads that created them (creation ownership)
    lock_creators: FxHashMap<LockId, ThreadId>,

    /// Maps threads to their parent threads (if any)
    thread_parents: FxHashMap<ThreadId, ThreadId>,

    /// Set of all threads that have been seen
    threads: FxHashSet<ThreadId>,

    /// Set of all locks that have been seen
    locks: FxHashSet<LockId>,
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
            lock_owners: FxHashMap::default(),
            thread_attempts: FxHashMap::default(),
            lock_creators: FxHashMap::default(),
            thread_parents: FxHashMap::default(),
            threads: FxHashSet::default(),
            locks: FxHashSet::default(),
        }
    }

    /// Handle thread spawn event by adding it to the threads set
    /// If parent_thread_id is provided, record the parent-child relationship
    pub fn update_thread_spawn(&mut self, thread_id: ThreadId, parent_thread_id: Option<ThreadId>) {
        self.threads.insert(thread_id);

        // Record parent thread if provided
        if let Some(parent_id) = parent_thread_id {
            self.thread_parents.insert(thread_id, parent_id);
        }
    }

    /// Handle thread exit event by removing it and handling resources it owned
    pub fn update_thread_exit(&mut self, thread_id: ThreadId) {
        // Remove thread from threads set
        self.threads.remove(&thread_id);

        // Find locks created by this thread that should be cleaned up
        let mut locks_to_destroy = Vec::new();
        for (&lock_id, &creator) in &self.lock_creators {
            if creator == thread_id {
                locks_to_destroy.push(lock_id);
            }
        }

        // Clean up locks created by this thread
        for lock_id in &locks_to_destroy {
            // Remove from all tracking structures
            self.lock_creators.remove(lock_id);
            self.lock_owners.remove(lock_id);
            self.locks.remove(lock_id);

            // Remove from all thread attempts
            for attempts in self.thread_attempts.values_mut() {
                attempts.remove(lock_id);
            }
        }

        // Find locks owned (but not created) by this thread that should be released
        let mut locks_to_release = Vec::new();
        for (&lock_id, &owner) in &self.lock_owners {
            if owner == thread_id && !locks_to_destroy.contains(&lock_id) {
                locks_to_release.push(lock_id);
            }
        }

        // Release locks owned by this thread
        for lock_id in locks_to_release {
            self.lock_owners.remove(&lock_id);
        }

        // Remove thread from attempts
        self.thread_attempts.remove(&thread_id);

        // Remove thread from parent tracking
        self.thread_parents.remove(&thread_id);

        // Update all threads that had this thread as a parent
        // This handles the case of "orphan" threads
        let orphaned_threads: Vec<ThreadId> = self
            .thread_parents
            .iter()
            .filter_map(|(&t, &p)| if p == thread_id { Some(t) } else { None })
            .collect();

        for orphan in orphaned_threads {
            self.thread_parents.remove(&orphan);
        }
    }

    /// Handle lock creation event by adding it to the locks set
    /// Also record which thread created the lock
    pub fn update_lock_create(&mut self, lock_id: LockId, creator_thread_id: ThreadId) {
        self.locks.insert(lock_id);
        self.lock_creators.insert(lock_id, creator_thread_id);
    }

    /// Handle lock destruction event by removing it from tracking
    pub fn update_lock_destroy(&mut self, lock_id: LockId) {
        // Remove the lock from all tracking structures
        self.locks.remove(&lock_id);
        self.lock_owners.remove(&lock_id);
        self.lock_creators.remove(&lock_id);

        // Remove from all thread attempts
        for attempts in self.thread_attempts.values_mut() {
            attempts.remove(&lock_id);
        }
    }

    /// Update the graph state based on a lock event
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread involved
    /// * `lock_id` - ID of the lock involved
    /// * `event` - Type of event that occurred
    pub fn update_lock_event(&mut self, thread_id: ThreadId, lock_id: LockId, event: Events) {
        // Always track the thread involved in the current event
        self.threads.insert(thread_id);

        // The lock should already be in the locks set from creation
        // This is a fallback in case it's not
        if !self.locks.contains(&lock_id) {
            self.locks.insert(lock_id);
        }

        match event {
            Events::Attempt => {
                // Add this attempt to the thread's attempts set
                self.thread_attempts
                    .entry(thread_id)
                    .or_default()
                    .insert(lock_id);
            }
            Events::Acquired => {
                // Record ownership of the lock
                self.lock_owners.insert(lock_id, thread_id);

                // Remove from attempts since it's now acquired
                if let Some(attempts) = self.thread_attempts.get_mut(&thread_id) {
                    attempts.remove(&lock_id);
                    // If thread has no more attempts, clean up
                    if attempts.is_empty() {
                        self.thread_attempts.remove(&thread_id);
                    }
                }
            }
            Events::Released => {
                // Remove ownership only if this thread owns it
                if self.lock_owners.get(&lock_id) == Some(&thread_id) {
                    self.lock_owners.remove(&lock_id);
                }
            }
            _ => {} // Spawn and Exit are handled separately
        }
    }

    /// Check if a lock was created by a specific thread
    #[allow(dead_code)]
    pub fn was_lock_created_by(&self, lock_id: LockId, thread_id: ThreadId) -> bool {
        self.lock_creators.get(&lock_id) == Some(&thread_id)
    }

    /// Get the parent of a thread, if any
    #[allow(dead_code)]
    pub fn get_thread_parent(&self, thread_id: ThreadId) -> Option<ThreadId> {
        self.thread_parents.get(&thread_id).copied()
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

/// Update graph logger with thread event
///
/// # Arguments
/// * `thread_id` - ID of the thread involved
/// * `parent_thread_id` - Optional ID of the parent thread that created this thread
/// * `event` - Type of event (Spawn or Exit)
pub fn update_thread(thread_id: ThreadId, parent_thread_id: Option<ThreadId>, event: Events) {
    if let Ok(mut logger) = GLOBAL_GRAPH_LOGGER.lock() {
        match event {
            Events::Spawn => logger.update_thread_spawn(thread_id, parent_thread_id),
            Events::Exit => logger.update_thread_exit(thread_id),
            _ => {} // Other events are handled by update_graph
        }
    }
}

/// Update graph logger with lock creation or destruction event
///
/// # Arguments
/// * `lock_id` - ID of the lock involved
/// * `creator_thread_id` - ID of the thread creating the lock (for Spawn event)
/// * `event` - Type of event (Spawn or Exit)
pub fn update_lock(lock_id: LockId, creator_thread_id: Option<ThreadId>, event: Events) {
    if let Ok(mut logger) = GLOBAL_GRAPH_LOGGER.lock() {
        match event {
            Events::Spawn => {
                if let Some(thread_id) = creator_thread_id {
                    logger.update_lock_create(lock_id, thread_id);
                } else {
                    // Fallback to thread ID 0 if no creator specified
                    logger.update_lock_create(lock_id, 0);
                }
            }
            Events::Exit => logger.update_lock_destroy(lock_id),
            _ => {} // Other events are handled by update_graph
        }
    }
}

/// Update the global graph logger with a lock event
///
/// # Arguments
/// * `thread_id` - ID of the thread involved
/// * `lock_id` - ID of the lock involved
/// * `event` - Type of event (Attempt, Acquired, or Released)
pub fn update_graph(thread_id: ThreadId, lock_id: LockId, event: Events) {
    if let Ok(mut logger) = GLOBAL_GRAPH_LOGGER.lock() {
        logger.update_lock_event(thread_id, lock_id, event);
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

/// Check if a lock was created by a specific thread
#[allow(dead_code)]
pub fn was_lock_created_by(lock_id: LockId, thread_id: ThreadId) -> bool {
    if let Ok(logger) = GLOBAL_GRAPH_LOGGER.lock() {
        logger.was_lock_created_by(lock_id, thread_id)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Events;

    #[test]
    fn test_lock_creation_and_destruction() {
        let mut logger = GraphLogger::new();

        // Create a lock with a creator thread
        logger.update_lock_create(10, 5);

        // Verify it's tracked
        assert!(logger.locks.contains(&10));
        assert!(logger.was_lock_created_by(10, 5));

        // Verify it appears in the graph
        let state = logger.get_current_state();
        assert!(state.locks.contains(&10));

        // Destroy the lock
        logger.update_lock_destroy(10);

        // Verify it's removed
        assert!(!logger.locks.contains(&10));
    }

    #[test]
    fn test_thread_parent_relationship() {
        let mut logger = GraphLogger::new();

        // Create parent thread
        logger.update_thread_spawn(5, None);

        // Create child thread with parent
        logger.update_thread_spawn(10, Some(5));

        // Verify parent-child relationship
        assert_eq!(logger.get_thread_parent(10), Some(5));

        // Exit parent thread
        logger.update_thread_exit(5);

        // Child's parent reference should be removed
        assert_eq!(logger.get_thread_parent(10), None);
    }

    #[test]
    fn test_thread_exit_cleans_up_created_locks() {
        let mut logger = GraphLogger::new();

        // Create a thread
        logger.update_thread_spawn(1, None);

        // Thread creates a lock
        logger.update_lock_create(10, 1);

        // Verify they're both tracked
        assert!(logger.threads.contains(&1));
        assert!(logger.locks.contains(&10));
        assert!(logger.was_lock_created_by(10, 1));

        // Thread exits - should clean up its created locks
        logger.update_thread_exit(1);

        // Thread and lock should be removed
        assert!(!logger.threads.contains(&1));
        assert!(!logger.locks.contains(&10));
    }

    #[test]
    fn test_thread_exit_releases_non_created_locks() {
        let mut logger = GraphLogger::new();

        // Create two threads
        logger.update_thread_spawn(1, None);
        logger.update_thread_spawn(2, None);

        // Thread 1 creates a lock
        logger.update_lock_create(10, 1);

        // Thread 2 acquires the lock
        logger.update_lock_event(2, 10, Events::Acquired);

        // Verify the lock is owned by thread 2 but created by thread 1
        assert!(logger.lock_owners.get(&10) == Some(&2));
        assert!(logger.was_lock_created_by(10, 1));

        // Thread 2 exits - should release the lock but not destroy it
        logger.update_thread_exit(2);

        // Thread 2 should be removed, lock should still exist
        assert!(!logger.threads.contains(&2));
        assert!(logger.locks.contains(&10));
        assert!(logger.lock_owners.get(&10).is_none()); // No owner

        // Thread 1 exits - should now destroy the lock
        logger.update_thread_exit(1);

        // Both threads and the lock should be removed
        assert!(!logger.threads.contains(&1));
        assert!(!logger.locks.contains(&10));
    }

    #[test]
    fn test_multiple_locks_with_mixed_ownership() {
        let mut logger = GraphLogger::new();

        // Create two threads
        logger.update_thread_spawn(1, None);
        logger.update_thread_spawn(2, None);

        // Thread 1 creates lock 10 and 11
        logger.update_lock_create(10, 1);
        logger.update_lock_create(11, 1);

        // Thread 2 creates lock 20
        logger.update_lock_create(20, 2);

        // Thread 1 acquires lock 20 (owned by 1, created by 2)
        logger.update_lock_event(1, 20, Events::Acquired);

        // Thread 2 acquires lock 10 (owned by 2, created by 1)
        logger.update_lock_event(2, 10, Events::Acquired);

        // Thread 1 exits - should clean up locks 10, 11 (created) and release lock 20
        logger.update_thread_exit(1);

        // Locks 10 and 11 should be removed because they were created by thread 1
        assert!(!logger.locks.contains(&10));
        assert!(!logger.locks.contains(&11));

        // Lock 20 should still exist since it was created by thread 2
        assert!(logger.locks.contains(&20));
        assert!(logger.lock_owners.get(&20).is_none()); // But no longer owned

        // Thread 2 exits - should clean up lock 20
        logger.update_thread_exit(2);

        // All locks should be gone
        assert!(!logger.locks.contains(&20));
    }
}
