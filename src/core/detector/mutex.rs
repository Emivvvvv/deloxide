use crate::core::detector::{DISPATCHER, GLOBAL_DETECTOR};
#[cfg(feature = "stress-test")]
use crate::core::stress::{
    on_lock_attempt as stress_on_lock_attempt, on_lock_release as stress_on_lock_release,
};
use crate::core::{Detector, Events, get_current_thread_id};
use crate::{DeadlockInfo, LockId, ThreadId};
use chrono::Utc;

impl Detector {
    /// Register a lock creation
    ///
    /// This method is called when a new mutex is created. It records which thread
    /// created the mutex for proper resource tracking.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created lock
    /// * `creator_id` - Optional ID of the thread that created this lock
    pub fn on_lock_create(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, Some(creator), Events::Spawn);
        }
    }

    /// Register a lock destruction
    ///
    /// This method is called when a mutex is being destroyed. It cleans up
    /// all references to the lock in the detector's data structures.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the lock being destroyed
    pub fn on_lock_destroy(&mut self, lock_id: LockId) {
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
            logger.log_lock_event(lock_id, None, Events::Exit);
        }

        // purge from all held-lock sets
        for holds in self.thread_holds.values_mut() {
            holds.remove(&lock_id);
        }
    }

    /// Register a lock attempt by a thread
    ///
    /// This method is called when a thread attempts to acquire a mutex. It records
    /// the attempt in the thread-lock relationship graph and checks for potential
    /// deadlock cycles.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire the lock
    /// * `lock_id` - ID of the lock being attempted
    pub fn on_lock_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::Attempt);
        }

        #[cfg(feature = "stress-test")]
        {
            if self.stress_mode != crate::core::stress::StressMode::None {
                if let Some(config) = &self.stress_config {
                    let held_locks = self
                        .thread_holds
                        .get(&thread_id)
                        .map(|set| set.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default();

                    stress_on_lock_attempt(
                        self.stress_mode,
                        thread_id,
                        lock_id,
                        &held_locks,
                        config,
                    );
                }
            }
        }

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
                    let info = DeadlockInfo {
                        thread_cycle: cycle.clone(),
                        thread_waiting_for_locks: self
                            .thread_waits_for
                            .iter()
                            .map(|(&t, &l)| (t, l))
                            .collect(),
                        timestamp: Utc::now().to_rfc3339(),
                    };

                    // Send deadlock info to dispatcher
                    DISPATCHER.send(info);
                }
            }
        }
    }

    /// Register successful lock acquisition by a thread
    ///
    /// This method is called when a thread successfully acquires a mutex. It updates
    /// the ownership information and clears any wait-for edges in the graph.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the lock
    /// * `lock_id` - ID of the lock that was acquired
    pub fn on_lock_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::Acquired);
        }

        // Update ownership
        self.mutex_owners.insert(lock_id, thread_id);
        self.thread_waits_for.remove(&thread_id);

        // Remove thread from wait graph
        self.wait_for_graph.remove_thread(thread_id);

        // Record held lock
        self.thread_holds
            .entry(thread_id)
            .or_default()
            .insert(lock_id);
    }

    /// Register lock release by a thread
    ///
    /// This method is called when a thread releases a mutex. It updates the ownership
    /// information in the detector's data structures.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the lock
    /// * `lock_id` - ID of the lock being released
    pub fn on_lock_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
        if let Some(logger) = &self.logger {
            logger.log_interaction_event(thread_id, lock_id, Events::Released);
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
        {
            if self.stress_mode != crate::core::stress::StressMode::None {
                if let Some(config) = &self.stress_config {
                    stress_on_lock_release(self.stress_mode, thread_id, lock_id, config);
                }
            }
        }
    }
}

/// Register a lock creation with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the created lock
/// * `creator_id` - Optional ID of the thread that created this lock
pub fn on_lock_create(lock_id: LockId, creator_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_lock_create(lock_id, creator_id);
}

/// Register a lock destruction with the global detector
///
/// # Arguments
/// * `lock_id` - ID of the lock being destroyed
pub fn on_lock_destroy(lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_lock_destroy(lock_id);
}

/// Register a lock attempt with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread attempting to acquire the lock
/// * `lock_id` - ID of the lock being attempted
pub fn on_lock_attempt(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_lock_attempt(thread_id, lock_id);
}

/// Register a lock acquisition with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread that acquired the lock
/// * `lock_id` - ID of the lock that was acquired
pub fn on_lock_acquired(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_lock_acquired(thread_id, lock_id);
}

/// Register a lock release with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the thread releasing the lock
/// * `lock_id` - ID of the lock being released
pub fn on_lock_release(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_lock_release(thread_id, lock_id);
}
