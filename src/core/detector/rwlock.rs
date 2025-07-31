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
    /// This method is called when a new RwLock is created. It records which thread
    /// created the RwLock for proper resource tracking and logging.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the created RwLock
    /// * `creator_id` - Optional ID of the thread that created this RwLock
    pub fn on_rwlock_create(&mut self, lock_id: LockId, creator_id: Option<ThreadId>) {
        let creator = creator_id.unwrap_or_else(get_current_thread_id);
        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, Some(creator), Events::Spawn);
        }
    }

    /// Register RwLock destruction
    ///
    /// This method is called when an RwLock is destroyed. It cleans up
    /// all references to the RwLock in the detector's data structures.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the RwLock being destroyed
    pub fn on_rwlock_destroy(&mut self, lock_id: LockId) {
        // Remove ownership (both read and write)
        self.rwlock_writer.remove(&lock_id);
        self.rwlock_readers.remove(&lock_id);

        // Remove from all held-lock sets
        for holds in self.thread_holds.values_mut() {
            holds.remove(&lock_id);
        }

        if let Some(logger) = &self.logger {
            logger.log_lock_event(lock_id, None, Events::Exit);
        }
    }

    /// Register a read lock attempt by a thread (supports stress testing and deadlock detection)
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire a read lock
    /// * `lock_id` - ID of the RwLock being attempted
    pub fn on_rw_read_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
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

    /// Register a write lock attempt by a thread (supports stress testing and deadlock detection)
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread attempting to acquire a write lock
    /// * `lock_id` - ID of the RwLock being attempted
    pub fn on_rw_write_attempt(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
    pub fn on_rw_read_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
        self.wait_for_graph.remove_thread(thread_id);
    }

    /// Register a successful write lock acquisition by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread that acquired the write lock
    /// * `lock_id` - ID of the RwLock
    pub fn on_rw_write_acquired(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
        self.wait_for_graph.remove_thread(thread_id);
    }

    /// Register a read lock release by a thread
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread releasing the read lock
    /// * `lock_id` - ID of the RwLock being released
    pub fn on_rw_read_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
    pub fn on_rw_write_release(&mut self, thread_id: ThreadId, lock_id: LockId) {
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
}

/// Register an RwLock creation with the global detector
pub fn on_rwlock_create(lock_id: LockId, creator_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rwlock_create(lock_id, creator_id);
}

/// Register RwLock destruction with the global detector
pub fn on_rwlock_destroy(lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rwlock_destroy(lock_id);
}

/// Register an RwLock read attempt with the global detector
pub fn on_rw_read_attempt(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_read_attempt(thread_id, lock_id);
}

/// Register an RwLock read acquisition with the global detector
pub fn on_rw_read_acquired(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_read_acquired(thread_id, lock_id);
}

/// Register an RwLock read release with the global detector
pub fn on_rw_read_release(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_read_release(thread_id, lock_id);
}

/// Register an RwLock write attempt with the global detector
pub fn on_rw_write_attempt(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_write_attempt(thread_id, lock_id);
}

/// Register an RwLock write acquisition with the global detector
pub fn on_rw_write_acquired(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_write_acquired(thread_id, lock_id);
}

/// Register a RwLock write release with the global detector
pub fn on_rw_write_release(thread_id: ThreadId, lock_id: LockId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_rw_write_release(thread_id, lock_id);
}
