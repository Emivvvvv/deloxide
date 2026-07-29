//! RwLock tracking and integration with the Deloxide detector
//!
//! This module defines all the RwLock-related hooks and Detector methods needed for
//! deadlock detection and logging of RwLock operations (read and write).

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::detector::deadlock_handling;
use crate::core::logger;
use crate::core::types::DeadlockInfo;
use crate::core::{Detector, Events, WaitIntent, WaitMode, get_current_thread_id};
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

    pub(crate) fn try_read_nonblocking<T, F>(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
        try_acquire_fn: F,
    ) -> Option<T>
    where
        F: FnOnce() -> Option<T>,
    {
        logger::log_interaction_event(thread_id, lock_id, Events::RwReadAttempt);
        let acquired = try_acquire_fn();
        if acquired.is_some() {
            self.rwlock_readers
                .entry(lock_id)
                .or_default()
                .entry(thread_id)
                .and_modify(|count| *count += 1)
                .or_insert(1);
            #[cfg(feature = "lock-order-graph")]
            self.thread_holds
                .entry(thread_id)
                .or_default()
                .insert(lock_id);
            logger::log_interaction_event(thread_id, lock_id, Events::RwReadAcquired);
            if self.lock_waiters.contains_key(&lock_id) {
                self.refresh_waiters_for_lock(lock_id);
            }
        }
        acquired
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
            .entry(thread_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        #[cfg(feature = "lock-order-graph")]
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        self.clear_wait_intent(thread_id);

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
        let mut still_holds_read = false;
        if let Some(readers) = self.rwlock_readers.get_mut(&lock_id) {
            if let Some(count) = readers.get_mut(&thread_id) {
                *count -= 1;
                if *count == 0 {
                    readers.remove(&thread_id);
                }
            }
            still_holds_read = readers.contains_key(&thread_id);
            if readers.is_empty() {
                self.rwlock_readers.remove(&lock_id);
            }
        }

        #[cfg(feature = "lock-order-graph")]
        if !still_holds_read && let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        // Remove stale edges for all threads waiting on this lock
        // (e.g. writers waiting for this reader)
        if !still_holds_read && let Some(waiters) = self.lock_waiters.get(&lock_id) {
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

        if let Some(writer) = potential_writer {
            self.rwlock_writer.insert(lock_id, writer);
        }

        let has_readers = self
            .rwlock_readers
            .get(&lock_id)
            .is_some_and(|readers| !readers.is_empty());
        let has_writer = self.rwlock_writer.contains_key(&lock_id);
        if (has_readers || has_writer)
            && let Some(cycle) =
                self.register_wait(thread_id, WaitIntent::new(lock_id, WaitMode::RwWrite))
        {
            return self.validated_deadlock_info(cycle);
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
        self.clear_wait_intent(thread_id);
        if let Some(cycle) = self.refresh_waiters_for_lock(lock_id)
            && let Some(info) = self.validated_deadlock_info(cycle)
        {
            deadlock_info = Some(info);
        }

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

pub fn try_read<T, F>(thread_id: ThreadId, lock_id: LockId, try_acquire_fn: F) -> Option<T>
where
    F: FnOnce() -> Option<T>,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.try_read_nonblocking(thread_id, lock_id, try_acquire_fn)
}

pub fn acquire_read_slow_with_recheck<T, F, H>(
    thread_id: ThreadId,
    lock_id: LockId,
    try_acquire: F,
    writer_hint: H,
) -> (Option<T>, Option<DeadlockInfo>)
where
    F: FnOnce() -> Option<T>,
    H: FnOnce() -> Option<ThreadId>,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    if let Some(acquired) = try_acquire() {
        detector.complete_read(thread_id, lock_id);
        return (Some(acquired), None);
    }

    if let Some(writer) = writer_hint() {
        detector.rwlock_writer.insert(lock_id, writer);
    }
    let cycle = detector.register_wait(thread_id, WaitIntent::new(lock_id, WaitMode::RwRead));
    let info = cycle.and_then(|cycle| detector.validated_deadlock_info(cycle));
    (None, info)
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

pub fn acquire_write_slow_with_recheck<T, F, H>(
    thread_id: ThreadId,
    lock_id: LockId,
    try_acquire: F,
    writer_hint: H,
) -> (Option<T>, Option<DeadlockInfo>)
where
    F: FnOnce() -> Option<T>,
    H: FnOnce() -> Option<ThreadId>,
{
    #[cfg(feature = "stress-test")]
    {
        let delay = {
            let detector = GLOBAL_DETECTOR.lock();
            detector.calculate_stress_delay(thread_id, lock_id)
        };
        if let Some(duration) = delay {
            thread::sleep(duration);
        }
    }

    let mut detector = GLOBAL_DETECTOR.lock();
    if let Some(acquired) = try_acquire() {
        let info = detector.complete_write(thread_id, lock_id);
        return (Some(acquired), info);
    }

    let info = detector.acquire_write_slow(thread_id, lock_id, writer_hint());
    (None, info)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_upgrade_registers_self_cycle() {
        let mut detector = Detector::new();
        detector.rwlock_readers.entry(10).or_default().insert(1, 1);

        let info = detector
            .acquire_write_slow(1, 10, None)
            .expect("blocking upgrade must be reported");

        assert_eq!(info.thread_cycle, vec![1]);
        assert_eq!(info.thread_waiting_for_locks, vec![(1, 10)]);
    }

    #[test]
    fn failed_nonblocking_read_leaves_no_wait_state() {
        let mut detector = Detector::new();
        detector.rwlock_writer.insert(10, 2);

        let result = detector.try_read_nonblocking(1, 10, || None::<()>);

        assert!(result.is_none());
        assert!(!detector.thread_waits_for.contains_key(&1));
        assert!(!detector.lock_waiters.contains_key(&10));
        assert!(detector.wait_for_graph.edges.get(&1).is_none());
    }

    #[test]
    fn recursive_read_release_preserves_remaining_hold() {
        let mut detector = Detector::new();
        detector.rwlock_readers.entry(10).or_default().insert(1, 2);

        detector.release_read(1, 10);
        assert_eq!(detector.rwlock_readers[&10][&1], 1);

        detector.release_read(1, 10);
        assert!(!detector.rwlock_readers.contains_key(&10));
    }

    #[test]
    #[cfg(feature = "lock-order-graph")]
    fn recursive_read_release_preserves_lock_order_hold_until_last_guard() {
        let mut detector = Detector::new();
        detector.complete_read(1, 10);
        detector.complete_read(1, 10);
        detector.register_wait(2, WaitIntent::new(10, WaitMode::RwWrite));
        assert_eq!(detector.wait_for_graph.outgoing(2), vec![1]);

        detector.release_read(1, 10);

        assert_eq!(detector.rwlock_readers[&10][&1], 1);
        assert!(detector.thread_holds[&1].contains(&10));
        assert_eq!(detector.wait_for_graph.outgoing(2), vec![1]);

        detector.release_read(1, 10);

        assert!(!detector.rwlock_readers.contains_key(&10));
        assert!(!detector.thread_holds.contains_key(&1));
        assert!(detector.wait_for_graph.outgoing(2).is_empty());
    }
}
