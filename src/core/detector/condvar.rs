//! Condvar Tracking and Integration with Deloxide Detector
//!
//! This module defines all the Condvar-related hooks and Detector methods needed for
//! deadlock detection and logging of Condvar operations (wait, notify). It bridges
//! condvar operations with mutex operations to ensure correct cycle detection.

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::types::{CondvarId, LockId, ThreadId};
use crate::core::{Detector, Events, get_current_thread_id};
use std::collections::VecDeque;

impl Detector {
    /// Register a condvar creation
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the created condition variable
    pub fn create_condvar(&mut self, condvar_id: CondvarId) {
        // Initialize the wait queue for this condvar
        self.cv_waiters.insert(condvar_id, VecDeque::new());

        self.log_if_enabled(|logger| {
            logger.log_lock_event(
                condvar_id,
                Some(get_current_thread_id()),
                Events::CondvarSpawn,
            );
        });
    }

    /// Register condvar destruction
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being destroyed
    pub fn destroy_condvar(&mut self, condvar_id: CondvarId) {
        // Clear wait queue
        self.cv_waiters.remove(&condvar_id);

        // Clear any thread wait mappings for this condvar
        self.thread_wait_cv
            .retain(|_, &mut (cv_id, _)| cv_id != condvar_id);

        if let Some(logger) = &self.logger {
            logger.log_lock_event(condvar_id, None, Events::CondvarExit);
        }
    }

    /// Register the beginning of a condvar wait operation
    ///
    /// This method is called when a thread begins waiting on a condition variable.
    /// It tracks which threads are waiting on which condvars and which mutex they
    /// will need to reacquire.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread beginning to wait
    /// * `condvar_id` - ID of the condition variable being waited on
    /// * `mutex_id` - ID of the mutex that will be reacquired after the wait
    pub fn begin_wait(&mut self, thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
        // Add thread to the wait queue for this condvar
        if let Some(queue) = self.cv_waiters.get_mut(&condvar_id) {
            queue.push_back((thread_id, mutex_id));
        } else {
            self.cv_waiters
                .insert(condvar_id, VecDeque::from([(thread_id, mutex_id)]));
        }

        // Track what this thread is waiting for
        self.thread_wait_cv
            .insert(thread_id, (condvar_id, mutex_id));

        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, condvar_id, Events::CondvarWaitBegin);
        }
    }

    /// Register a condvar notify_one operation
    ///
    /// This method is called when a thread calls notify_one on a condition variable.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being notified
    /// * `notifier_id` - ID of the thread performing the notification
    pub fn notify_one(&mut self, condvar_id: CondvarId, notifier_id: ThreadId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(notifier_id, condvar_id, Events::CondvarNotifyOne);
        }

        // Wake one waiter if any exist
        if let Some(queue) = self.cv_waiters.get_mut(&condvar_id)
            && let Some((waiter_thread, mutex_id)) = queue.pop_front()
        {
            // Mark as woken (for diagnostics)
            self.cv_woken.insert(waiter_thread);

            self.on_mutex_attempt_synthetic_immediate(waiter_thread, mutex_id);
        }
    }

    /// Register a condvar notify_all operation
    ///
    /// This method is called when a thread calls notify_all on a condition variable.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being notified
    /// * `notifier_id` - ID of the thread performing the notification
    pub fn notify_all(&mut self, condvar_id: CondvarId, notifier_id: ThreadId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(notifier_id, condvar_id, Events::CondvarNotifyAll);
        }

        // Wake all waiters
        let waiters_to_wake: Vec<(ThreadId, LockId)> =
            if let Some(queue) = self.cv_waiters.get_mut(&condvar_id) {
                queue.drain(..).collect()
            } else {
                Vec::new()
            };

        for (waiter_thread, mutex_id) in waiters_to_wake {
            // Mark as woken (for diagnostics)
            self.cv_woken.insert(waiter_thread);

            self.on_mutex_attempt_synthetic_immediate(waiter_thread, mutex_id);
        }
    }

    /// Register the end of a condvar wait operation
    ///
    /// This method is called when a thread's wait operation completes (mutex reacquired).
    /// It cleans up the wait tracking and finalizes the synthetic mutex acquisition.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread whose wait is ending
    /// * `condvar_id` - ID of the condition variable that was waited on
    /// * `mutex_id` - ID of the mutex that was reacquired
    pub fn end_wait(&mut self, thread_id: ThreadId, condvar_id: CondvarId, _mutex_id: LockId) {
        // Remove from thread wait tracking
        self.thread_wait_cv.remove(&thread_id);

        // Remove from woken set if present
        self.cv_woken.remove(&thread_id);

        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, condvar_id, Events::CondvarWaitEnd);
        }
    }

    /// Synthetic mutex attempt for condvar operations (immediate processing)
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the mutex
    /// * `lock_id` - ID of the mutex being attempted
    ///
    /// # Note
    /// This method does NOT attempt actual lock acquisition - it only sets up
    /// wait-for edges and performs cycle detection. The actual acquisition will
    /// happen when the woken thread calls the mutex wrapper's lock() method.
    fn on_mutex_attempt_synthetic_immediate(&mut self, thread_id: ThreadId, lock_id: LockId) {
        // Check for lock order violations (only if graph exists and holding other locks)
        #[cfg(feature = "lock-order-graph")]
        let lock_order_violation = if self.lock_order_graph.is_some()
            && self.thread_holds.get(&thread_id).map_or(0, |h| h.len()) >= 1
        {
            self.check_lock_order_violation(thread_id, lock_id)
        } else {
            None
        };
        #[cfg(not(feature = "lock-order-graph"))]
        let _lock_order_violation: Option<Vec<LockId>> = None;

        if let Some(&owner) = self.mutex_owners.get(&lock_id) {
            // Mutex is owned - set up wait-for edge
            self.thread_waits_for.insert(thread_id, lock_id);

            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner) {
                // Apply common lock filter
                let filtered_cycle = self.filter_cycle_by_common_locks(&cycle);

                if !filtered_cycle.is_empty() {
                    // Real deadlock detected!
                    self.handle_detected_deadlock(cycle);
                }
            }
        }

        // Report lock order violation if detected
        #[cfg(feature = "lock-order-graph")]
        if let Some(lock_cycle) = lock_order_violation {
            self.handle_lock_order_violation(thread_id, lock_id, lock_cycle);
        }

        // Keep thread in cv_woken set - it will be cleared when actual acquisition happens
    }
}

/// Register a condvar creation with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the created condition variable
pub fn create_condvar(condvar_id: CondvarId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.create_condvar(condvar_id);
}

/// Register condvar destruction with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being destroyed
pub fn destroy_condvar(condvar_id: CondvarId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.destroy_condvar(condvar_id);
}

/// Register the beginning of a condvar wait with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread beginning to wait
/// * `condvar_id` - ID of the condition variable being waited on
/// * `mutex_id` - ID of the mutex that will be reacquired after the wait
pub fn begin_wait(thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.begin_wait(thread_id, condvar_id, mutex_id);
}

/// Register a condvar notify_one with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being notified
/// * `notifier_id` - ID of the thread performing the notification
pub fn notify_one(condvar_id: CondvarId, notifier_id: ThreadId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.notify_one(condvar_id, notifier_id);
}

/// Register a condvar notify_all with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being notified
/// * `notifier_id` - ID of the thread performing the notification
pub fn notify_all(condvar_id: CondvarId, notifier_id: ThreadId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.notify_all(condvar_id, notifier_id);
}

/// Register the end of a condvar wait with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread whose wait is ending
/// * `condvar_id` - ID of the condition variable that was waited on
/// * `mutex_id` - ID of the mutex that was reacquired
pub fn end_wait(thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.end_wait(thread_id, condvar_id, mutex_id);
}
