//! RwLock tracking and integration with the Deloxide detector
//!
//! This module defines all the RwLock-related hooks and Detector methods needed for
//! deadlock detection and logging of RwLock operations (read and write).

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::detector::deadlock_handling;
use crate::core::logger;
use crate::core::types::DeadlockInfo;
use crate::core::{Detector, Events, get_current_thread_id};
use crate::{LockId, ThreadId};
#[cfg(feature = "stress-test")]
use std::thread;
impl Detector {
    /// Register an RwLock creation
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created RwLock
    /// * `creator_id` - Optional ID of the thread that created this RwLock
    pub fn create_rwlock(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        logger::log_lock_event(lock_id, Some(creator), Events::RwSpawn);
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
        #[cfg(feature = "lock-order-graph")]
        if let Some(graph) = &mut self.lock_order_graph {
            graph.remove_lock(lock_id);
        }

        // Remove from lock waiters
        self.lock_waiters.remove(&lock_id);
        logger::log_lock_event(lock_id, None, Events::RwExit);
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
        potential_writer: Option<ThreadId>,
        try_acquire_fn: F,
    ) -> Result<Option<T>, Vec<ThreadId>>
    where
        F: FnOnce() -> Option<T>,
    {
        // Log the attempt
        logger::log_interaction_event(thread_id, lock_id, Events::RwReadAttempt);

        // Apply stress testing while holding detector lock

        // Check if there's a writer (Global State OR Atomic Hint)
        let effective_writer = self
            .rwlock_writer
            .get(&lock_id)
            .copied()
            .or(potential_writer);

        if let Some(writer) = effective_writer {
            self.thread_waits_for.insert(thread_id, lock_id);
            self.lock_waiters
                .entry(lock_id)
                .or_default()
                .insert(thread_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                // Apply common lock filter
                let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                if !filtered_cycle.is_empty() {
                    // Real deadlock detected!
                    return Err(cycle);
                }
            }

            // Writer exists but no deadlock - will need to block
            return Ok(None);
        }

        // No writer - try to acquire read lock while still holding GLOBAL_DETECTOR
        if let Some(guard) = try_acquire_fn() {
            // Success! Update detector state immediately
            self.rwlock_readers
                .entry(lock_id)
                .or_default()
                .insert(thread_id);
            #[cfg(feature = "lock-order-graph")]
            self.thread_holds
                .entry(thread_id)
                .or_default()
                .insert(lock_id);

            // NOTE: Read locks do NOT clear wait edges!
            // Multiple readers can coexist, so the thread stays in the graph
            // for potential upgrade deadlock detection.
            self.thread_waits_for.remove(&thread_id);

            // Log acquisition
            logger::log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);

            Ok(Some(guard))
        } else {
            // try_read failed - a writer must have acquired it
            // Set up wait-for edges for the blocking read() that will follow
            if let Some(&writer) = self.rwlock_writer.get(&lock_id) {
                self.thread_waits_for.insert(thread_id, lock_id);
                self.lock_waiters
                    .entry(lock_id)
                    .or_default()
                    .insert(thread_id);

                if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                    let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);
                    if !filtered_cycle.is_empty() {
                        return Err(cycle);
                    }
                }
            }

            Ok(None)
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
        #[cfg(feature = "lock-order-graph")]
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        self.thread_waits_for.remove(&thread_id);
        if let Some(waiters) = self.lock_waiters.get_mut(&lock_id) {
            waiters.remove(&thread_id);
            if waiters.is_empty() {
                self.lock_waiters.remove(&lock_id);
            }
        }

        // Log acquisition
        logger::log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);
    }

    /// Register a read lock release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the read lock
    /// * `lock_id` - ID of the RwLock being released
    pub fn release_read(&mut self, thread_id: ThreadId, lock_id: LockId) {
        logger::log_interaction_event(thread_id, lock_id, Events::RwReadReleased);
        if let Some(readers) = self.rwlock_readers.get_mut(&lock_id) {
            readers.remove(&thread_id);
            if readers.is_empty() {
                self.rwlock_readers.remove(&lock_id);
            }
        }

        #[cfg(feature = "lock-order-graph")]
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        // Remove stale edges for all threads waiting on this lock
        // (e.g. writers waiting for this reader)
        if let Some(waiters) = self.lock_waiters.get(&lock_id) {
            for &waiter in waiters {
                self.wait_for_graph.remove_edge(waiter, thread_id);
            }
        }

        #[cfg(feature = "stress-test")]
        self.stress_on_lock_release(thread_id, lock_id);
    }

    /// Register a write lock release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the write lock
    /// * `lock_id` - ID of the RwLock being released
    pub fn release_write(&mut self, thread_id: ThreadId, lock_id: LockId) {
        logger::log_interaction_event(thread_id, lock_id, Events::RwWriteReleased);
        if self.rwlock_writer.get(&lock_id) == Some(&thread_id) {
            self.rwlock_writer.remove(&lock_id);
        }
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        // Remove stale edges for all threads waiting on this lock
        if let Some(waiters) = self.lock_waiters.get(&lock_id) {
            for &waiter in waiters {
                self.wait_for_graph.remove_edge(waiter, thread_id);
            }
        }

        #[cfg(feature = "stress-test")]
        self.stress_on_lock_release(thread_id, lock_id);
    }

    /// Register a slow-path write lock acquisition attempt (Optimized)
    ///
    /// This method should be called by the RwLock wrapper only when the optimistic
    /// `try_write` has failed. It uses the `potential_writer` hint to detect
    /// deadlocks even if the current writer is using the Fast Path.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the write lock
    /// * `lock_id` - ID of the RwLock being attempted
    /// * `potential_writer` - The thread ID observed holding the write lock (if any)
    pub fn acquire_write_slow(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
        potential_writer: Option<ThreadId>,
    ) -> Option<DeadlockInfo> {
        // Log the attempt
        logger::log_interaction_event(thread_id, lock_id, Events::RwWriteAttempt);

        #[cfg(feature = "lock-order-graph")]
        if self.lock_order_graph.is_some()
            && self.thread_holds.get(&thread_id).map_or(0, |h| h.len()) >= 1
            && let Some(lock_cycle) = self.check_lock_order_violation(thread_id, lock_id)
        {
            return Some(self.extract_lock_order_violation_info(thread_id, lock_id, lock_cycle));
        }

        // Check for conflicting readers (Global State)
        if let Some(readers) = self.rwlock_readers.get(&lock_id) {
            for &reader in readers {
                if reader != thread_id {
                    self.thread_waits_for.insert(thread_id, lock_id);
                    self.lock_waiters
                        .entry(lock_id)
                        .or_default()
                        .insert(thread_id);
                    if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, reader) {
                        // No common lock filtering for upgrades (Reader->Writer deps)
                        return Some(self.extract_deadlock_info(cycle));
                    }
                }
            }
        }

        // Check for conflicting writer (Global State or Atomic Hint)
        let effective_writer = self.rwlock_writer.get(&lock_id).copied().or_else(|| {
            if let Some(writer) = potential_writer {
                // Trust the atomic hint from the wrapper.
                // We rely on the wrapper to verify this edge if a deadlock is detected.
                return Some(writer);
            }
            None
        });

        if let Some(writer) = effective_writer
            && writer != thread_id
        {
            self.thread_waits_for.insert(thread_id, lock_id);
            self.lock_waiters
                .entry(lock_id)
                .or_default()
                .insert(thread_id);
            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, writer) {
                return Some(self.extract_deadlock_info(cycle));
            }
        }
        None
    }

    /// Update detector state after blocking write lock acquisition
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the write lock
    /// * `lock_id` - ID of the RwLock
    pub fn complete_write(&mut self, thread_id: ThreadId, lock_id: LockId) -> Option<DeadlockInfo> {
        self.rwlock_writer.insert(lock_id, thread_id);

        #[allow(unused_mut)]
        let mut deadlock_info = None;

        #[cfg(feature = "lock-order-graph")]
        if self.lock_order_graph.is_some()
            && self.thread_holds.get(&thread_id).map_or(0, |h| h.len()) >= 1
            && let Some(lock_cycle) = self.check_lock_order_violation(thread_id, lock_id)
        {
            deadlock_info =
                Some(self.extract_lock_order_violation_info(thread_id, lock_id, lock_cycle));
        }

        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        // Clear wait-for edges
        self.thread_waits_for.remove(&thread_id);
        if let Some(waiters) = self.lock_waiters.get_mut(&lock_id) {
            waiters.remove(&thread_id);
            if waiters.is_empty() {
                self.lock_waiters.remove(&lock_id);
            }
        }
        self.wait_for_graph.clear_wait_edges(thread_id);

        // Log acquisition
        logger::log_interaction_event(thread_id, lock_id, Events::RwWriteAcquired);

        deadlock_info
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
    // 1. Calculate stress delay (holding lock)
    #[cfg(feature = "stress-test")]
    let delay = {
        let detector = GLOBAL_DETECTOR.lock();
        detector.calculate_stress_delay(thread_id, lock_id)
    };

    // 2. Apply delay (without lock)
    #[cfg(feature = "stress-test")]
    if let Some(duration) = delay {
        thread::sleep(duration);
    }

    // 3. Proceed with detection (re-acquiring lock)
    let (result, deadlock_info) = {
        let mut detector = GLOBAL_DETECTOR.lock();
        match detector.attempt_read(thread_id, lock_id, None, try_acquire_fn) {
            Ok(val) => (val, None),
            Err(cycle) => (None, Some(detector.extract_deadlock_info(cycle))),
        }
    };

    if let Some(info) = deadlock_info {
        deadlock_handling::process_deadlock(info);
    }

    result
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

/// Register a slow-path write lock acquisition attempt with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the write lock
/// * `lock_id` - ID of the RwLock being attempted
/// * `potential_writer` - The thread ID observed holding the write lock
pub fn acquire_write_slow(
    thread_id: ThreadId,
    lock_id: LockId,
    potential_writer: Option<ThreadId>,
) -> Option<DeadlockInfo> {
    // 1. Calculate stress delay (holding lock)
    #[cfg(feature = "stress-test")]
    let delay = {
        let detector = GLOBAL_DETECTOR.lock();
        detector.calculate_stress_delay(thread_id, lock_id)
    };

    // 2. Apply delay (without lock)
    #[cfg(feature = "stress-test")]
    if let Some(duration) = delay {
        thread::sleep(duration);
    }

    // 3. Proceed with detection (re-acquiring lock)
    {
        let mut detector = GLOBAL_DETECTOR.lock();
        detector.acquire_write_slow(thread_id, lock_id, potential_writer)
    }
}

/// Complete write lock acquisition after blocking
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the write lock
/// * `lock_id` - ID of the RwLock
pub fn complete_write(thread_id: ThreadId, lock_id: LockId) {
    let deadlock_info = {
        let mut detector = GLOBAL_DETECTOR.lock();
        detector.complete_write(thread_id, lock_id)
    };

    if let Some(info) = deadlock_info {
        deadlock_handling::process_deadlock(info);
    }
}
