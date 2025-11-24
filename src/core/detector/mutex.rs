//! Mutex Tracking and Integration with Deloxide Detector
//!
//! This module defines all the Mutex-related hooks and Detector methods needed for
//! deadlock detection and logging of Mutex operations (acquisition and release).

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::detector::deadlock_handling;
use crate::core::logger;
use crate::core::types::DeadlockInfo;
use crate::core::{Detector, Events, get_current_thread_id};
use crate::{LockId, ThreadId};
#[cfg(feature = "stress-test")]
use std::thread;

impl Detector {
    /// Register a mutex creation
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created mutex
    /// * `creator_id` - Optional ID of the thread that created this mutex
    pub fn create_mutex(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        logger::log_lock_event(lock_id, Some(creator), Events::MutexSpawn);
    }

    /// Register mutex destruction
    ///
    /// # Arguments
    /// * `lock_id` - ID of the mutex being destroyed
    pub fn destroy_mutex(&mut self, lock_id: LockId) {
        // remove ownership
        self.mutex_owners.remove(&lock_id);
        // clear any pending wait-for for this lock
        for attempts in self.thread_waits_for.values_mut() {
            if *attempts == lock_id {
                *attempts = 0;
            }
        }
        self.thread_waits_for.retain(|_, &mut l| l != 0);

        logger::log_lock_event(lock_id, None, Events::MutexExit);

        // purge from all held-lock sets
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
    }

    /// Register a slow-path mutex acquisition attempt (Optimized)
    ///
    /// This method should be called by the Mutex wrapper only when the optimistic
    /// `try_lock` has failed. It uses the provided `potential_owner` hint (read from
    /// the wrapper's atomic state) to reconstruct the dependency graph even if the
    /// current owner acquired the lock via the fast path (and thus isn't in the global map).
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the mutex
    /// * `lock_id` - ID of the mutex being attempted
    /// * `potential_owner` - The thread ID observed holding the lock (if any)
    pub fn acquire_slow(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
        potential_owner: Option<ThreadId>,
    ) -> Option<Vec<ThreadId>> {
        // Log the attempt
        logger::log_interaction_event(thread_id, lock_id, Events::MutexAttempt);

        // Apply stress testing


        // Determine the effective owner.
        // Priority: Global state > Atomic hint (if validated or waking from Condvar).
        let effective_owner = self.mutex_owners.get(&lock_id).copied().or_else(|| {
            if let Some(owner) = potential_owner {
                // Trust the atomic hint from the wrapper.
                // We rely on the wrapper to verify this edge if a deadlock is detected,
                // to filter out stale edges from Fast Path releases.
                return Some(owner);
            }
            None
        });

        if let Some(owner) = effective_owner {
            // We are waiting for this owner
            self.thread_waits_for.insert(thread_id, lock_id);
            self.lock_waiters
                .entry(lock_id)
                .or_default()
                .insert(thread_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                if !filtered_cycle.is_empty() {
                    return Some(cycle);
                }
            }
        }
        None
    }

    /// Complete mutex acquisition after blocking
    ///
    /// Updates detector state after a blocking lock acquisition.
    /// Call this after attempt_acquire() returns None and you use a blocking lock().
    pub fn complete_acquire(
        &mut self,
        thread_id: ThreadId,
        lock_id: LockId,
    ) -> Option<DeadlockInfo> {
        self.mutex_owners.insert(lock_id, thread_id);

        // Remove from lock waiters
        if let Some(waiters) = self.lock_waiters.get_mut(&lock_id) {
            waiters.remove(&thread_id);
            if waiters.is_empty() {
                self.lock_waiters.remove(&lock_id);
            }
        }

        self.thread_waits_for.remove(&thread_id);
        self.wait_for_graph.clear_wait_edges(thread_id);

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

        logger::log_interaction_event(thread_id, lock_id, Events::MutexAcquired);

        deadlock_info
    }

    /// Register mutex release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the mutex
    /// * `lock_id` - ID of the mutex being released
    pub fn release_mutex(&mut self, thread_id: ThreadId, lock_id: LockId) {
        logger::log_interaction_event(thread_id, lock_id, Events::MutexReleased);
        if self.mutex_owners.get(&lock_id) == Some(&thread_id) {
            self.mutex_owners.remove(&lock_id);
        }
        // remove from held-locks
        if let Some(holds) = self.thread_holds.get_mut(&thread_id) {
            holds.remove(&lock_id);
            if holds.is_empty() {
                self.thread_holds.remove(&thread_id);
            }
        }

        // Remove stale edges for all threads waiting on this lock
        if let Some(waiters) = self.lock_waiters.get(&lock_id) {
            for &waiter in waiters {
                // Each waiter currently has an edge to 'thread_id' (the current owner)
                // We must remove it because 'thread_id' no longer owns the lock.
                // The waiter is now waiting for "no one" (or the next owner).
                // We don't know the next owner yet, so we just clear the edge.
                self.wait_for_graph.remove_edge(waiter, thread_id);
            }
        }

        // Apply post-release stress testing if enabled
        #[cfg(feature = "stress-test")]
        self.stress_on_lock_release(thread_id, lock_id);
    }
}

/// Register a mutex creation with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the created mutex
/// * `creator_id` - Optional ID of the thread that created this mutex
pub fn create_mutex(lock_id: LockId, creator_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.create_mutex(lock_id, creator_id);
}

/// Register mutex destruction with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the mutex being destroyed
pub fn destroy_mutex(lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.destroy_mutex(lock_id);
}

/// Register a mutex release with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread releasing the mutex
/// * `lock_id` - ID of the mutex being released
pub fn release_mutex(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.release_mutex(thread_id, lock_id);
}

/// Complete mutex acquisition after blocking
///
/// Called after a blocking lock() call completes.
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the mutex
/// * `lock_id` - ID of the mutex that was acquired
pub fn complete_acquire(thread_id: ThreadId, lock_id: LockId) {
    let deadlock_info = {
        let mut detector = GLOBAL_DETECTOR.lock();
        detector.complete_acquire(thread_id, lock_id)
    };

    if let Some(info) = deadlock_info {
        deadlock_handling::process_deadlock(info);
    }
}

/// Register a slow-path mutex acquisition attempt with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the mutex
/// * `lock_id` - ID of the mutex being attempted
/// * `potential_owner` - The thread ID observed holding the lock
pub fn acquire_slow(
    thread_id: ThreadId,
    lock_id: LockId,
    potential_owner: Option<ThreadId>,
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
        let cycle = detector.acquire_slow(thread_id, lock_id, potential_owner);
        cycle.map(|cycle| detector.extract_deadlock_info(cycle))
    }
}
