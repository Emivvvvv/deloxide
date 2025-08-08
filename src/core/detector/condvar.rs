//! Condvar Tracking and Integration with Deloxide Detector
//!
//! This module defines all the Condvar-related hooks and Detector methods needed for
//! deadlock detection and logging of Condvar operations (wait, notify). It bridges
//! condvar operations with mutex operations to ensure correct cycle detection.

use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::{Detector, Events, get_current_thread_id};
use crate::core::types::{CondvarId, LockId, ThreadId};
use std::collections::VecDeque;

impl Detector {
    /// Register a condvar creation
    ///
    /// This method is called when a new condition variable is created. It initializes
    /// tracking structures for the condvar.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the created condition variable
    pub fn on_condvar_create(&mut self, condvar_id: CondvarId) {
        // Initialize the wait queue for this condvar
        self.cv_waiters.insert(condvar_id, VecDeque::new());
        
        self.log_if_enabled(|logger| {
            logger.log_lock_event(condvar_id, Some(get_current_thread_id()), Events::Spawn);
        });
    }

    /// Register condvar destruction
    ///
    /// This method is called when a condition variable is being destroyed. It cleans up
    /// all references to the condvar in the detector's data structures.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being destroyed
    pub fn on_condvar_destroy(&mut self, condvar_id: CondvarId) {
        // Clear wait queue
        self.cv_waiters.remove(&condvar_id);
        
        // Clear any thread wait mappings for this condvar
        self.thread_wait_cv.retain(|_, &mut (cv_id, _)| cv_id != condvar_id);
        
        if let Some(logger) = &self.logger {
            logger.log_lock_event(condvar_id, None, Events::Exit);
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
    pub fn on_condvar_wait_begin(&mut self, thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
        // Add thread to the wait queue for this condvar
        if let Some(queue) = self.cv_waiters.get_mut(&condvar_id) {
            queue.push_back((thread_id, mutex_id));
        } else {
            self.cv_waiters.insert(condvar_id, VecDeque::from([(thread_id, mutex_id)]));
        }
        
        // Track what this thread is waiting for
        self.thread_wait_cv.insert(thread_id, (condvar_id, mutex_id));
        
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, condvar_id, Events::CondvarWaitBegin);
        }
    }

    /// Register a condvar notify_one operation
    ///
    /// This method is called when a thread calls notify_one on a condition variable.
    /// It wakes one waiter and synthesizes a mutex attempt for that thread.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being notified
    /// * `notifier_id` - ID of the thread performing the notification
    pub fn on_condvar_notify_one(&mut self, condvar_id: CondvarId, notifier_id: ThreadId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(notifier_id, condvar_id, Events::CondvarNotifyOne);
        }
        
        // Wake one waiter if any exist
        if let Some(queue) = self.cv_waiters.get_mut(&condvar_id) {
            if let Some((waiter_thread, mutex_id)) = queue.pop_front() {
                // Mark as woken (for diagnostics)
                self.cv_woken.insert(waiter_thread);
                
                // Synthesize a mutex attempt for the woken thread
                // This creates the necessary wait-for graph edge while they compete to reacquire
                self.on_mutex_attempt_synthetic(waiter_thread, mutex_id);
            }
        }
    }

    /// Register a condvar notify_all operation
    ///
    /// This method is called when a thread calls notify_all on a condition variable.
    /// It wakes all waiters and synthesizes mutex attempts for all of them.
    ///
    /// # Arguments
    /// * `condvar_id` - ID of the condition variable being notified
    /// * `notifier_id` - ID of the thread performing the notification
    pub fn on_condvar_notify_all(&mut self, condvar_id: CondvarId, notifier_id: ThreadId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(notifier_id, condvar_id, Events::CondvarNotifyAll);
        }
        
        // Wake all waiters
        let waiters_to_wake: Vec<(ThreadId, LockId)> = if let Some(queue) = self.cv_waiters.get_mut(&condvar_id) {
            queue.drain(..).collect()
        } else {
            Vec::new()
        };
        
        // Process each woken waiter
        for (waiter_thread, mutex_id) in waiters_to_wake {
            // Mark as woken (for diagnostics)
            self.cv_woken.insert(waiter_thread);
            
            // Synthesize a mutex attempt for each woken thread
            // This creates the necessary wait-for graph edges while they compete to reacquire
            self.on_mutex_attempt_synthetic(waiter_thread, mutex_id);
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
    pub fn on_condvar_wait_end(&mut self, thread_id: ThreadId, condvar_id: CondvarId, _mutex_id: LockId) {
        // Remove from thread wait tracking
        self.thread_wait_cv.remove(&thread_id);
        
        // Remove from woken set if present
        self.cv_woken.remove(&thread_id);
        
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, condvar_id, Events::CondvarWaitEnd);
        }
    }

    /// Synthetic mutex attempt for condvar operations
    ///
    /// This method simulates a mutex attempt without going through the normal
    /// mutex wrapper. It's used when a condvar notify operation wakes a thread
    /// that will need to reacquire its mutex.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the mutex
    /// * `lock_id` - ID of the mutex being attempted
    fn on_mutex_attempt_synthetic(&mut self, thread_id: ThreadId, lock_id: LockId) {
        // Mark this as a synthetic attempt so it doesn't get removed prematurely
        self.cv_woken.insert(thread_id);
        
        // Call the same logic as normal mutex attempt but mark it as synthetic
        self.on_mutex_attempt(thread_id, lock_id);
    }
}

/// Register a condvar creation with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the created condition variable
pub fn on_condvar_create(condvar_id: CondvarId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_create(condvar_id);
}

/// Register condvar destruction with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being destroyed
pub fn on_condvar_destroy(condvar_id: CondvarId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_destroy(condvar_id);
}

/// Register the beginning of a condvar wait with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread beginning to wait
/// * `condvar_id` - ID of the condition variable being waited on
/// * `mutex_id` - ID of the mutex that will be reacquired after the wait
pub fn on_wait_begin(thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_wait_begin(thread_id, condvar_id, mutex_id);
}

/// Register a condvar notify_one with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being notified
/// * `notifier_id` - ID of the thread performing the notification
pub fn on_notify_one(condvar_id: CondvarId, notifier_id: ThreadId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_notify_one(condvar_id, notifier_id);
}

/// Register a condvar notify_all with the global detector
///
/// # Arguments
/// * `condvar_id` - ID of the condition variable being notified
/// * `notifier_id` - ID of the thread performing the notification
pub fn on_notify_all(condvar_id: CondvarId, notifier_id: ThreadId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_notify_all(condvar_id, notifier_id);
}

/// Register the end of a condvar wait with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread whose wait is ending
/// * `condvar_id` - ID of the condition variable that was waited on
/// * `mutex_id` - ID of the mutex that was reacquired
pub fn on_wait_end(thread_id: ThreadId, condvar_id: CondvarId, mutex_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_condvar_wait_end(thread_id, condvar_id, mutex_id);
}