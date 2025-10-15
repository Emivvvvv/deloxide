//! Mutex Tracking and Integration with Deloxide Detector
//!
//! This module defines all the Mutex-related hooks and Detector methods needed for
//! deadlock detection and logging of Mutex operations (acquisition and release).

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::{Detector, Events, get_current_thread_id};
use crate::{LockId, ThreadId};

impl Detector {
    /// Register a mutex creation
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created mutex
    /// * `creator_id` - Optional ID of the thread that created this mutex
    pub fn create_mutex(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, Some(creator), Events::MutexSpawn);
        }
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

        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, None, Events::MutexExit);
        }

        // purge from all held-lock sets
        for holds in self.thread_holds.values_mut() {
            holds.remove(&lock_id);
        }

        // Remove from lock order graph if it exists
        if let Some(graph) = &mut self.lock_order_graph {
            graph.remove_lock(lock_id);
        }
    }

    /// Attempt to acquire a mutex with atomic deadlock detection
    ///
    /// Performs deadlock detection and attempts non-blocking acquisition atomically.
    /// If successful, returns the guard. If the lock is busy or a deadlock is detected,
    /// returns None and the caller should use blocking acquisition with complete_acquire().
    pub fn attempt_acquire<T, F>(
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
            logger.log_interaction_event(thread_id, lock_id, Events::MutexAttempt);
        });

        // Apply stress testing WHILE holding detector lock
        // This ensures timing delays happen atomically with detection
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

        if let Some(&owner) = self.mutex_owners.get(&lock_id) {
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                if !filtered_cycle.is_empty() {
                    self.handle_detected_deadlock(cycle);
                    return None;
                }
            }

            return None;
        }

        // Report lock order violation if detected
        if let Some(lock_cycle) = lock_order_violation {
            self.handle_lock_order_violation(thread_id, lock_id, lock_cycle);
        }

        if let Some(guard) = try_acquire_fn() {
            self.mutex_owners.insert(lock_id, thread_id);

            if !self.cv_woken.contains(&thread_id) {
                self.thread_waits_for.remove(&thread_id);
                self.wait_for_graph.clear_wait_edges(thread_id);
            }

            self.thread_holds
                .entry(thread_id)
                .or_default()
                .insert(lock_id);

            self.log_if_enabled(|logger| {
                logger.log_interaction_event(thread_id, lock_id, Events::MutexAcquired);
            });

            Some(guard)
        } else {
            // Lock became busy, set up wait-for edges for blocking acquisition
            if let Some(&owner) = self.mutex_owners.get(&lock_id) {
                self.thread_waits_for.insert(thread_id, lock_id);

                if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                    let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                    if !filtered_cycle.is_empty() {
                        self.handle_detected_deadlock(cycle);
                    }
                }
            }

            None
        }
    }

    /// Complete mutex acquisition after blocking
    ///
    /// Updates detector state after a blocking lock acquisition.
    /// Call this after attempt_acquire() returns None and you use a blocking lock().
    pub fn complete_acquire(&mut self, thread_id: ThreadId, lock_id: LockId) {
        self.mutex_owners.insert(lock_id, thread_id);

        if !self.cv_woken.contains(&thread_id) {
            self.thread_waits_for.remove(&thread_id);
            self.wait_for_graph.clear_wait_edges(thread_id);
        }

        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);

        self.log_if_enabled(|logger| {
            logger.log_interaction_event(thread_id, lock_id, Events::MutexAcquired);
        });
    }

    /// Register mutex release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the mutex
    /// * `lock_id` - ID of the mutex being released
    pub fn release_mutex(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::MutexReleased);
        }
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

/// Attempt to acquire a mutex with atomic deadlock detection
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the mutex
/// * `lock_id` - ID of the mutex being attempted
/// * `try_acquire_fn` - Closure that attempts non-blocking lock acquisition
///
/// # Returns
/// * `Some(T)` - Lock was acquired successfully
/// * `None` - Lock is busy, deadlock detected, or acquisition failed
pub fn attempt_acquire<T, F>(thread_id: ThreadId, lock_id: LockId, try_acquire_fn: F) -> Option<T>
where
    F: FnOnce() -> Option<T>,
{
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.attempt_acquire(thread_id, lock_id, try_acquire_fn)
}

/// Complete mutex acquisition after blocking
///
/// Called after a blocking lock() call completes.
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the mutex
/// * `lock_id` - ID of the mutex that was acquired
pub fn complete_acquire(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.complete_acquire(thread_id, lock_id);
}
