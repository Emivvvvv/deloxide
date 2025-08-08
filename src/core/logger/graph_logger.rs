use crate::core::types::{Events, LockId, ThreadId};
use fxhash::{FxHashMap, FxHashSet};
use serde::Serialize;

/// Represents a link between a thread and a lock
#[derive(Debug, Serialize, Clone)]
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
///
/// This structure provides a snapshot of all threads, locks, and their relationships
/// at a particular point in time. It is used for visualization and analysis.
#[derive(Debug, Serialize, Clone)]
pub struct GraphState {
    /// All thread IDs in the system
    pub threads: Vec<ThreadId>,
    /// All lock IDs in the system
    pub locks: Vec<LockId>,
    /// Links between threads and locks representing ownership and attempts
    pub links: Vec<GraphLink>,
}

/// Maintains the current state of threads and locks in the system
///
/// The GraphLogger is responsible for tracking the complete state of thread-lock
/// relationships. Each EventLogger instance maintains its own GraphLogger to provide
/// independent tracking of different execution contexts.
pub struct GraphLogger {
    /// Maps locks to the threads that currently own them (runtime ownership)
    mutex_owners: FxHashMap<LockId, ThreadId>,
    /// Maps RwLocks to the threads that currently hold read locks on them
    rwlock_readers: FxHashMap<LockId, FxHashSet<ThreadId>>,
    /// Maps RwLocks to the thread that currently holds the write lock on them
    rwlock_writer: FxHashMap<LockId, ThreadId>,
    /// Maps threads to the locks they're attempting to acquire
    thread_attempts: FxHashMap<ThreadId, FxHashSet<LockId>>,
    /// Maps threads to the condvars they're waiting on
    condvar_waiters: FxHashMap<ThreadId, LockId>,
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
    /// Create a new graph logger instance
    ///
    /// # Returns
    /// A new GraphLogger with empty tracking structures
    pub fn new() -> Self {
        GraphLogger {
            mutex_owners: FxHashMap::default(),
            rwlock_readers: FxHashMap::default(),
            rwlock_writer: FxHashMap::default(),
            thread_attempts: FxHashMap::default(),
            condvar_waiters: FxHashMap::default(),
            lock_creators: FxHashMap::default(),
            thread_parents: FxHashMap::default(),
            threads: FxHashSet::default(),
            locks: FxHashSet::default(),
        }
    }

    /// Handle thread spawn event by adding it to the threads set
    ///
    /// If parent_thread_id is provided, record the parent-child relationship.
    /// This is used for tracking thread hierarchies and resource ownership.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the newly spawned thread
    /// * `parent_thread_id` - Optional ID of the parent thread that created this thread
    pub fn update_thread_spawn(&mut self, thread_id: ThreadId, parent_thread_id: Option<ThreadId>) {
        self.threads.insert(thread_id);

        // Record parent thread if provided
        if let Some(parent_id) = parent_thread_id {
            self.thread_parents.insert(thread_id, parent_id);
        }
    }

    /// Handle thread exit event by removing it and handling resources it owned
    ///
    /// This method performs cleanup for all resources associated with the exiting thread:
    /// - Removes the thread from tracking
    /// - Destroys locks created by the thread
    /// - Releases locks owned by the thread
    /// - Updates thread hierarchy for orphaned children
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the exiting thread
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
            self.mutex_owners.remove(lock_id);
            self.locks.remove(lock_id);

            // Remove from all thread attempts
            for attempts in self.thread_attempts.values_mut() {
                attempts.remove(lock_id);
            }
        }

        // Find locks owned (but not created) by this thread that should be released
        let mut locks_to_release = Vec::new();
        for (&lock_id, &owner) in &self.mutex_owners {
            if owner == thread_id && !locks_to_destroy.contains(&lock_id) {
                locks_to_release.push(lock_id);
            }
        }

        // Release locks owned by this thread
        for lock_id in locks_to_release {
            self.mutex_owners.remove(&lock_id);
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
    ///
    /// Also records which thread created the lock for proper resource tracking.
    ///
    /// # Arguments
    /// * `lock_id` - The ID of the newly created lock
    /// * `creator_thread_id` - The ID of the thread that created this lock
    pub fn update_lock_create(&mut self, lock_id: LockId, creator_thread_id: ThreadId) {
        self.locks.insert(lock_id);
        self.lock_creators.insert(lock_id, creator_thread_id);
    }

    /// Handle lock destruction event by removing it from tracking
    ///
    /// This method cleans up all references to the destroyed lock from all tracking structures.
    ///
    /// # Arguments
    /// * `lock_id` - The ID of the lock being destroyed
    pub fn update_lock_destroy(&mut self, lock_id: LockId) {
        // Remove the lock from all tracking structures
        self.locks.remove(&lock_id);
        self.mutex_owners.remove(&lock_id);
        self.lock_creators.remove(&lock_id);

        // Remove from all thread attempts
        for attempts in self.thread_attempts.values_mut() {
            attempts.remove(&lock_id);
        }
    }

    /// Update the graph state based on a lock event
    ///
    /// This method processes lock-related events (Attempt, Acquired, Released) and
    /// updates the internal state accordingly. It also ensures threads and locks
    /// are tracked even if they weren't explicitly created through spawn events.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread involved in the event
    /// * `lock_id` - ID of the lock involved in the event
    /// * `event` - The type of lock event that occurred
    pub fn update_lock_event(&mut self, thread_id: ThreadId, lock_id: LockId, event: Events) {
        // Always track the thread involved in the current event
        self.threads.insert(thread_id);

        // The lock should already be in the locks set from creation
        // This is a fallback in case it's not
        if !self.locks.contains(&lock_id) {
            self.locks.insert(lock_id);
        }

        match event {
            // Any attempt (mutex, rwlock, condvar) goes here:
            Events::MutexAttempt | Events::RwReadAttempt | Events::RwWriteAttempt /* | Events::CondvarWaitAttempt */ => {
                self.thread_attempts.entry(thread_id).or_default().insert(lock_id);
            }

            // Any successful acquisition (mutex, rwlock read, rwlock write):
            Events::MutexAcquired | Events::RwReadAcquired | Events::RwWriteAcquired => {
                // Remove attempt
                if let Some(attempts) = self.thread_attempts.get_mut(&thread_id) {
                    attempts.remove(&lock_id);
                    if attempts.is_empty() { self.thread_attempts.remove(&thread_id); }
                }
                // Record actual ownership in the right map:
                match event {
                    Events::MutexAcquired => { self.mutex_owners.insert(lock_id, thread_id); }
                    Events::RwReadAcquired => {
                        self.rwlock_readers.entry(lock_id).or_default().insert(thread_id);
                    }
                    Events::RwWriteAcquired => {
                        self.rwlock_writer.insert(lock_id, thread_id);
                    }
                    _ => {}
                }
            }

            // Release for any lock type
            Events::MutexReleased => {
                // Remove mutex ownership if this thread owns the lock
                if self.mutex_owners.get(&lock_id) == Some(&thread_id) {
                    self.mutex_owners.remove(&lock_id);
                }
            }
            Events::RwReadReleased => {
                if let Some(readers) = self.rwlock_readers.get_mut(&lock_id) {
                    readers.remove(&thread_id);
                    if readers.is_empty() { self.rwlock_readers.remove(&lock_id); }
                }
            }
            Events::RwWriteReleased => {
                if self.rwlock_writer.get(&lock_id) == Some(&thread_id) {
                    self.rwlock_writer.remove(&lock_id);
                }
            }

            // Condvar events
            Events::CondvarWaitBegin => {
                self.condvar_waiters.insert(thread_id, lock_id);
            }
            Events::CondvarWaitEnd => {
                self.condvar_waiters.remove(&thread_id);
            }

            // Condvar notifications don't affect thread-condvar relationships in the graph
            Events::CondvarNotifyOne | Events::CondvarNotifyAll => {
                // These are logged but don't change the graph state
            }

            _ => {}
        }
    }

    /// Generate the current graph state
    ///
    /// This method creates a snapshot of the current thread-lock relationships for
    /// visualization or analysis purposes.
    ///
    /// # Returns
    /// A GraphState structure containing all current threads, locks, and their relationships
    pub fn get_current_state(&self) -> GraphState {
        let mut links = Vec::new();

        // Mutex ownership
        for (&lock_id, &thread_id) in &self.mutex_owners {
            links.push(GraphLink {
                source: thread_id,
                target: lock_id,
                link_type: "Acquired".to_string(),
            });
        }

        // RwLock read ownership
        for (&lock_id, readers) in &self.rwlock_readers {
            for &thread_id in readers {
                links.push(GraphLink {
                    source: thread_id,
                    target: lock_id,
                    link_type: "Read".to_string(),
                });
            }
        }

        // RwLock write ownership
        for (&lock_id, &thread_id) in &self.rwlock_writer {
            links.push(GraphLink {
                source: thread_id,
                target: lock_id,
                link_type: "Write".to_string(),
            });
        }

        // Attempts for any lock type
        for (&thread_id, attempts) in &self.thread_attempts {
            for &lock_id in attempts {
                links.push(GraphLink {
                    source: thread_id,
                    target: lock_id,
                    link_type: "Attempt".to_string(),
                });
            }
        }

        // Condvar waits
        for (&thread_id, &condvar_id) in &self.condvar_waiters {
            links.push(GraphLink {
                source: thread_id,
                target: condvar_id,
                link_type: "Wait".to_string(),
            });
        }

        GraphState {
            threads: self.threads.iter().copied().collect(),
            locks: self.locks.iter().copied().collect(),
            links,
        }
    }

    /// Check if a lock was created by a specific thread
    ///
    /// # Arguments
    /// * `lock_id` - The ID of the lock to check
    /// * `thread_id` - The ID of the potential creator thread
    ///
    /// # Returns
    /// true if the thread created the lock, false otherwise
    #[cfg(test)]
    pub fn was_lock_created_by(&self, lock_id: LockId, thread_id: ThreadId) -> bool {
        self.lock_creators.get(&lock_id) == Some(&thread_id)
    }

    /// Get the parent of a thread, if any
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to check
    ///
    /// # Returns
    /// The parent thread ID if one exists, None otherwise
    #[cfg(test)]
    pub fn get_thread_parent(&self, thread_id: ThreadId) -> Option<ThreadId> {
        self.thread_parents.get(&thread_id).copied()
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
        logger.update_lock_event(2, 10, Events::MutexAcquired);

        // Verify the lock is owned by thread 2 but created by thread 1
        assert_eq!(logger.mutex_owners.get(&10), Some(&2));
        assert!(logger.was_lock_created_by(10, 1));

        // Thread 2 exits - should release the lock but not destroy it
        logger.update_thread_exit(2);

        // Thread 2 should be removed, lock should still exist
        assert!(!logger.threads.contains(&2));
        assert!(logger.locks.contains(&10));
        assert!(logger.mutex_owners.get(&10).is_none()); // No owner

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
        logger.update_lock_event(1, 20, Events::MutexAcquired);

        // Thread 2 acquires lock 10 (owned by 2, created by 1)
        logger.update_lock_event(2, 10, Events::MutexAcquired);

        // Thread 1 exits - should clean up locks 10, 11 (created) and release lock 20
        logger.update_thread_exit(1);

        // Locks 10 and 11 should be removed because they were created by thread 1
        assert!(!logger.locks.contains(&10));
        assert!(!logger.locks.contains(&11));

        // Lock 20 should still exist since it was created by thread 2
        assert!(logger.locks.contains(&20));
        assert!(logger.mutex_owners.get(&20).is_none()); // But no longer owned

        // Thread 2 exits - should clean up lock 20
        logger.update_thread_exit(2);

        // All locks should be gone
        assert!(!logger.locks.contains(&20));
    }

    #[test]
    fn test_attempt_and_acquisition_tracking() {
        let mut logger = GraphLogger::new();

        // Create thread and lock
        logger.update_thread_spawn(1, None);
        logger.update_lock_create(10, 1);

        // Thread attempts to acquire lock
        logger.update_lock_event(1, 10, Events::MutexAttempt);

        // Verify attempt is tracked
        assert!(logger.thread_attempts.get(&1).unwrap().contains(&10));

        // Thread acquires lock
        logger.update_lock_event(1, 10, Events::MutexAcquired);

        // Verify acquisition is tracked and attempt is removed
        assert!(logger.mutex_owners.get(&10) == Some(&1));
        assert!(logger.thread_attempts.get(&1).is_none()); // No more attempts

        // Thread releases lock
        logger.update_lock_event(1, 10, Events::MutexReleased);

        // Verify ownership is removed
        assert!(logger.mutex_owners.get(&10).is_none());
    }

    #[test]
    fn test_graph_state_generation() {
        let mut logger = GraphLogger::new();

        // Create complex scenario
        logger.update_thread_spawn(1, None);
        logger.update_thread_spawn(2, Some(1));
        logger.update_lock_create(10, 1);
        logger.update_lock_create(20, 2);

        // Create various relationships
        logger.update_lock_event(1, 20, Events::MutexAttempt);
        logger.update_lock_event(2, 10, Events::MutexAcquired);

        // Get graph state
        let state = logger.get_current_state();

        // Verify state contains expected elements
        assert_eq!(state.threads.len(), 2);
        assert_eq!(state.locks.len(), 2);
        assert_eq!(state.links.len(), 2);

        // Check links
        let attempt_link = state
            .links
            .iter()
            .find(|l| l.link_type == "Attempt")
            .unwrap();
        assert_eq!(attempt_link.source, 1);
        assert_eq!(attempt_link.target, 20);

        let acquired_link = state
            .links
            .iter()
            .find(|l| l.link_type == "Acquired")
            .unwrap();
        assert_eq!(acquired_link.source, 2);
        assert_eq!(acquired_link.target, 10);
    }
}
