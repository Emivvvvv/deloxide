//! RwLock tracking and integration with the Deloxide detector
//!
//! This module defines all the RwLock-related hooks and Detector methods needed for
//! deadlock detection and logging of RwLock operations (read and write).

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::{Detector, Events, get_current_thread_id};
use crate::{LockId, ThreadId};
impl Detector {
    /// Register an RwLock creation
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created RwLock
    /// * `creator_id` - Optional ID of the thread that created this RwLock
    pub fn create_rwlock(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, Some(creator), Events::RwSpawn);
        }
    }

    /// Register RwLock destruction
    ///
    /// # Arguments
    /// * `lock_id` - ID of the RwLock being destroyed
    pub fn destroy_rwlock(&mut self, lock_id: LockId) {
        // Remove ownership (both read and write)
        self.rwlock_writer.remove(&lock_id);
        self.rwlock_readers.remove(&lock_id);

        // Remove from all held-lock sets
        for holds in self.thread_holds.values_mut() {
            holds.remove(&lock_id);
        }

        // Remove from lock order graph if it exists
        if let Some(graph) = &mut self.lock_order_graph {
            graph.remove_lock(lock_id);
        }

        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, None, Events::RwExit);
        }
    }

    /// Register a read lock attempt (deprecated - use attempt_read instead)
    #[deprecated(note = "Use attempt_read for atomic detection and acquisition")]
    pub fn _on_rw_read_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwReadAttempt);
        }

        // --- Stress testing: random preemption or component-based (if enabled) ---
        #[cfg(feature = "stress-test")]
        self.stress_on_lock_attempt(thread_id, lock_id);

        // Wait-for graph: If a writer exists, we must wait for it.
        if let Some(&writer) = self.rwlock_writer.get(&lock_id) {
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                // This is where the deadlock is detected!
                self.handle_detected_deadlock(cycle)
            }
        }
    }

    /// Read lock attempt and try-acquire operation
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the read lock
    /// * `lock_id` - ID of the RwLock being attempted
    /// * `try_acquire_fn` - Closure that attempts non-blocking read lock acquisition
    ///
    /// # Returns
    /// * `Some(T)` - Read lock was acquired successfully
    /// * `None` - Lock is busy (writer exists), deadlock detected, or acquisition failed
    pub fn attempt_read<T, F>(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
        try_acquire_fn: F,
    ) -> Option<T>
    where
        F: FnOnce() -> Option<T>,
    {
        // Log the attempt
        self.log_if_enabled(|logger| {
            logger.log_interaction_event(thread_id, lock_id, Events::RwReadAttempt);
        });

        // Apply stress testing while holding detector lock
        #[cfg(feature = "stress-test")]
        self.stress_on_lock_attempt(thread_id, lock_id);

        // Check if there's a writer - if so, we'll have to wait
        if let Some(&writer) = self.rwlock_writer.get(&lock_id) {
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                // Apply common lock filter
                let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                if !filtered_cycle.is_empty() {
                    // Real deadlock detected!
                    self.handle_detected_deadlock(cycle);
                    return None;
                }
            }

            // Writer exists but no deadlock - will need to block
            return None;
        }

        // No writer - try to acquire read lock while still holding GLOBAL_DETECTOR
        if let Some(guard) = try_acquire_fn() {
            // Success! Update detector state immediately
            self.rwlock_readers
                .entry(lock_id)
                .or_default()
                .insert(thread_id);
            self.thread_holds
                .entry(thread_id)
                .or_default()
                .insert(lock_id);

            // NOTE: Read locks do NOT clear wait edges!
            // Multiple readers can coexist, so the thread stays in the graph
            // for potential upgrade deadlock detection.
            self.thread_waits_for.remove(&thread_id);

            // Log acquisition
            self.log_if_enabled(|logger| {
                logger.log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);
            });

            Some(guard)
        } else {
            // try_read failed - a writer must have acquired it
            // Set up wait-for edges for the blocking read() that will follow
            if let Some(&writer) = self.rwlock_writer.get(&lock_id) {
                self.thread_waits_for.insert(thread_id, lock_id);

                if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                    let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);
                    if !filtered_cycle.is_empty() {
                        self.handle_detected_deadlock(cycle);
                    }
                }
            }

            None
        }
    }

    /// Update detector state after blocking read lock acquisition
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the read lock
    /// * `lock_id` - ID of the RwLock
    pub fn complete_read(&mut self, thread_id: ThreadId, lock_id: LockId) {
        self.rwlock_readers
            .entry(lock_id)
            .or_default()
            .insert(thread_id);
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        self.thread_waits_for.remove(&thread_id);

        // Log acquisition
        self.log_if_enabled(|logger| {
            logger.log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);
        });
    }

    /// Register a write lock attempt (deprecated - use attempt_write instead)
    #[deprecated(note = "Use attempt_write for atomic detection and acquisition")]
    pub fn _on_rw_write_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwWriteAttempt);
        }

        #[cfg(feature = "stress-test")]
        self.stress_on_lock_attempt(thread_id, lock_id);

        // Wait for all *other* readers
        if let Some(readers) = self.rwlock_readers.get(&lock_id) {
            for &reader in readers {
                if reader != thread_id {
                    self.thread_waits_for.insert(thread_id, lock_id);
                    if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, reader) {
                        // This is where the deadlock is detected!
                        self.handle_detected_deadlock(cycle)
                    }
                }
            }
        }

        // Wait for the current writer, if any (shouldn’t happen during upgrade, but just in case)
        if let Some(&writer) = self.rwlock_writer.get(&lock_id)
            && writer != thread_id
        {
            self.thread_waits_for.insert(thread_id, lock_id);
            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                // This is where the deadlock is detected!
                self.handle_detected_deadlock(cycle)
            }
        }
    }

    /// Register a successful read lock acquisition by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the read lock
    /// * `lock_id` - ID of the RwLock
    #[deprecated(note = "Use complete_read for proper two-phase locking")]
    pub fn _on_rw_read_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);
        }
        self.rwlock_readers
            .entry(lock_id)
            .or_default()
            .insert(thread_id);
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        // Remove wait-for edges for this thread (done waiting)
        self.thread_waits_for.remove(&thread_id);
        self.wait_for_graph.clear_wait_edges(thread_id);
    }

    /// Register a successful write lock acquisition by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the write lock
    /// * `lock_id` - ID of the RwLock
    #[deprecated(note = "Use complete_write for proper two-phase locking")]
    pub fn _on_rw_write_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwWriteAcquired);
        }
        self.rwlock_writer.insert(lock_id, thread_id);
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        // Remove wait-for edges for this thread (done waiting)
        self.thread_waits_for.remove(&thread_id);
        self.wait_for_graph.clear_wait_edges(thread_id);
    }

    /// Register a read lock release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the read lock
    /// * `lock_id` - ID of the RwLock being released
    pub fn release_read(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwReadReleased);
        }
        if let Some(readers) = self.rwlock_readers.get_mut(&lock_id) {
            readers.remove(&thread_id);
            if readers.is_empty() {
                self.rwlock_readers.remove(&lock_id);
            }
        }
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }
    }

    /// Register a write lock release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the write lock
    /// * `lock_id` - ID of the RwLock being released
    pub fn release_write(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::RwWriteReleased);
        }
        if self.rwlock_writer.get(&lock_id) == Some(&thread_id) {
            self.rwlock_writer.remove(&lock_id);
        }
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        #[cfg(feature = "stress-test")]
        self.stress_on_lock_release(thread_id, lock_id);
    }

    /// Atomic write lock attempt and try-acquire operation
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the write lock
    /// * `lock_id` - ID of the RwLock being attempted
    /// * `try_acquire_fn` - Closure that attempts non-blocking write lock acquisition
    ///
    /// # Returns
    /// * `Some(T)` - Write lock was acquired successfully
    /// * `None` - Lock is busy (readers or writer exist), deadlock detected, or acquisition failed
    pub fn attempt_write<T, F>(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
        try_acquire_fn: F,
    ) -> Option<T>
    where
        F: FnOnce() -> Option<T>,
    {
        // Log the attempt
        self.log_if_enabled(|logger| {
            logger.log_interaction_event(thread_id, lock_id, Events::RwWriteAttempt);
        });

        // Apply stress testing while holding detector lock
        #[cfg(feature = "stress-test")]
        self.stress_on_lock_attempt(thread_id, lock_id);

        // Check for lock order violations (only if graph exists and holding other locks)
        let lock_order_violation = if self.lock_order_graph.is_some()
            && self.thread_holds.get(&thread_id).map_or(0, |h| h.len()) >= 1
        {
            self.check_lock_order_violation(thread_id, lock_id)
        } else {
            None
        };

        let mut has_conflicts = false;

        // Check for conflicting readers (all readers except ourselves)
        if let Some(readers) = self.rwlock_readers.get(&lock_id) {
            for &reader in readers {
                if reader != thread_id {
                    has_conflicts = true;
                    self.thread_waits_for.insert(thread_id, lock_id);

                    if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, reader) {
                        // NOTE: Do NOT apply common lock filter for RwLock upgrades!
                        // Multiple threads can hold read locks on the same RwLock,
                        // and that's exactly what causes upgrade deadlocks.
                        // The filter would incorrectly mark this as a false positive.
                        self.handle_detected_deadlock(cycle);
                        return None;
                    }
                }
            }
        }

        // Check for conflicting writer
        if let Some(&writer) = self.rwlock_writer.get(&lock_id)
            && writer != thread_id
        {
            has_conflicts = true;
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                // Real deadlock detected!
                self.handle_detected_deadlock(cycle);
                return None;
            }
        }

        if has_conflicts {
            // Conflicts exist but no deadlock - will need to block
            return None;
        }

        // Report lock order violation if detected
        if let Some(lock_cycle) = lock_order_violation {
            self.handle_lock_order_violation(thread_id, lock_id, lock_cycle);
        }

        // No conflicts - try to acquire write lock while still holding GLOBAL_DETECTOR
        if let Some(guard) = try_acquire_fn() {
            // Success! Update detector state immediately
            self.rwlock_writer.insert(lock_id, thread_id);
            self.thread_holds
                .entry(thread_id)
                .or_default()
                .insert(lock_id);

            // Clear wait-for edges
            self.thread_waits_for.remove(&thread_id);
            self.wait_for_graph.clear_wait_edges(thread_id);

            // Log acquisition
            self.log_if_enabled(|logger| {
                logger.log_interaction_event(thread_id, lock_id, Events::RwWriteAcquired);
            });

            Some(guard)
        } else {
            // try_write failed - readers or writer must have acquired it
            // Set up wait-for edges for the blocking write() that will follow
            self.thread_waits_for.insert(thread_id, lock_id);

            // Check for readers
            if let Some(readers) = self.rwlock_readers.get(&lock_id) {
                for &reader in readers {
                    if reader != thread_id
                        && let Some(cycle) = self.wait_for_graph.add_edge(thread_id, reader)
                    {
                        self.handle_detected_deadlock(cycle);
                        return None;
                    }
                }
            }

            // Check for writer
            if let Some(&writer) = self.rwlock_writer.get(&lock_id)
                && writer != thread_id
                && let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer)
            {
                self.handle_detected_deadlock(cycle);
            }

            None
        }
    }

    /// Update detector state after blocking write lock acquisition
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the write lock
    /// * `lock_id` - ID of the RwLock
    pub fn complete_write(&mut self, thread_id: ThreadId, lock_id: LockId) {
        self.rwlock_writer.insert(lock_id, thread_id);
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        // Clear wait-for edges
        self.thread_waits_for.remove(&thread_id);
        self.wait_for_graph.clear_wait_edges(thread_id);

        // Log acquisition
        self.log_if_enabled(|logger| {
            logger.log_interaction_event(thread_id, lock_id, Events::RwWriteAcquired);
        });
    }
}

/// Register an RwLock creation with the global detector
pub fn create_rwlock(lock_id: LockId, creator_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.create_rwlock(lock_id, creator_id);
}

/// Register RwLock destruction with the global detector
pub fn destroy_rwlock(lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.destroy_rwlock(lock_id);
}

/// Register an RwLock read release with the global detector
pub fn release_read(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.release_read(thread_id, lock_id);
}

/// Register a RwLock write release with the global detector
pub fn release_write(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.release_write(thread_id, lock_id);
}

/// Read lock attempt and try-acquire with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the read lock
/// * `lock_id` - ID of the RwLock being attempted
/// * `try_acquire_fn` - Closure that attempts non-blocking read lock acquisition
///
/// # Returns
/// * `Some(T)` - Read lock was acquired successfully
/// * `None` - Lock is busy, deadlock detected, or acquisition failed
pub fn attempt_read<T, F>(thread_id: ThreadId, lock_id: LockId, try_acquire_fn: F) -> Option<T>
where
    F: FnOnce() -> Option<T>,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.attempt_read(thread_id, lock_id, try_acquire_fn)
}

/// Complete read lock acquisition after blocking
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the read lock
/// * `lock_id` - ID of the RwLock
pub fn complete_read(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.complete_read(thread_id, lock_id);
}

/// Write lock attempt and try-acquire with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the write lock
/// * `lock_id` - ID of the RwLock being attempted
/// * `try_acquire_fn` - Closure that attempts non-blocking write lock acquisition
///
/// # Returns
/// * `Some(T)` - Write lock was acquired successfully
/// * `None` - Lock is busy, deadlock detected, or acquisition failed
pub fn attempt_write<T, F>(thread_id: ThreadId, lock_id: LockId, try_acquire_fn: F) -> Option<T>
where
    F: FnOnce() -> Option<T>,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.attempt_write(thread_id, lock_id, try_acquire_fn)
}

/// Complete write lock acquisition after blocking
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the write lock
/// * `lock_id` - ID of the RwLock
pub fn complete_write(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.complete_write(thread_id, lock_id);
}
