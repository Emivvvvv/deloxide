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
    /// This method is called when a new mutex is created. It records which thread
    /// created the mutex for proper resource tracking.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created mutex
    /// * `creator_id` - Optional ID of the thread that created this mutex
    pub fn on_mutex_create(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, Some(creator), Events::MutexSpawn);
        }
    }

    /// Register mutex destruction
    ///
    /// This method is called when a mutex is being destroyed. It cleans up
    /// all references to the mutex in the detector's data structures.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the mutex being destroyed
    pub fn on_mutex_destroy(&mut self, lock_id: LockId) {
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

        // Remove from lock order graph
        self.lock_order_graph.remove_lock(lock_id);
    }

    /// Register a mutex attempt by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the mutex
    /// * `lock_id` - ID of the mutex being attempted
    pub fn on_mutex_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::MutexAttempt);
        }

        #[cfg(feature = "stress-test")]
        self.stress_on_lock_attempt(thread_id, lock_id);

        // Only check lock order when holding 1+ locks
        let lock_order_violation = if self.thread_holds.get(&thread_id).map_or(0, |h| h.len()) >= 1
        {
            self.check_lock_order_violation(thread_id, lock_id)
        } else {
            None
        };

        // Check for actual wait-for cycles (traditional detection)
        if let Some(&owner) = self.mutex_owners.get(&lock_id) {
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                // Apply filter for common locks
                let mut iter = cycle.iter();
                let first = *iter.next().unwrap();
                let mut intersection = self.thread_holds.get(&first).cloned().unwrap_or_default();

                for &t in iter {
                    if let Some(holds) = self.thread_holds.get(&t) {
                        intersection = intersection.intersection(holds).copied().collect();
                    } else {
                        intersection.clear();
                        break;
                    }
                }

                // Only report if no common lock (i.e., false-alarm filter)
                if intersection.is_empty() {
                    self.handle_detected_deadlock(cycle);
                    return; // Reported via traditional detection
                }
            }
        }

        // Report lock order violations when lock is available
        if let Some(lock_cycle) = lock_order_violation
            && !self.mutex_owners.contains_key(&lock_id)
        {
            self.handle_lock_order_violation(thread_id, lock_id, lock_cycle);
        }
    }

    /// Register a successful mutex acquisition by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the mutex
    /// * `lock_id` - ID of the mutex that was acquired
    pub fn on_mutex_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::MutexAcquired);
        }

        // Update ownership
        self.mutex_owners.insert(lock_id, thread_id);

        // For synthetic attempts (condvar woken threads), don't remove wait-for edges immediately
        // This allows deadlock detection to see the full cycle
        if !self.cv_woken.contains(&thread_id) {
            self.thread_waits_for.remove(&thread_id);
            // Remove thread from wait graph
            self.wait_for_graph.remove_thread(thread_id);
        }

        // Record held lock
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);
    }

    /// Register mutex release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the mutex
    /// * `lock_id` - ID of the mutex being released
    pub fn on_mutex_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
pub fn on_mutex_create(lock_id: LockId, creator_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_mutex_create(lock_id, creator_id);
}

/// Register mutex destruction with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the mutex being destroyed
pub fn on_mutex_destroy(lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_mutex_destroy(lock_id);
}

/// Register a mutex attempt with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the mutex
/// * `lock_id` - ID of the mutex being attempted
pub fn on_mutex_attempt(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_mutex_attempt(thread_id, lock_id);
}

/// Register a mutex acquisition with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the mutex
/// * `lock_id` - ID of the mutex that was acquired
pub fn on_mutex_acquired(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_mutex_acquired(thread_id, lock_id);
}

/// Register a mutex release with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread releasing the mutex
/// * `lock_id` - ID of the mutex being released
pub fn on_mutex_release(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_mutex_release(thread_id, lock_id);
}
