use crate::core::Detector;
use crate::core::detector::DISPATCHER;
use crate::{DeadlockInfo, LockId, ThreadId};
use chrono::Utc;

impl Detector {
    pub fn handle_detected_deadlock(&self, cycle: Vec<ThreadId>) {
        let info = DeadlockInfo {
            thread_cycle: cycle.clone(),
            thread_waiting_for_locks: self
                .thread_waits_for
                .iter()
                .map(|(&t, &l)| (t, l))
                .collect(),
            lock_order_cycle: None,
            timestamp: Utc::now().to_rfc3339(),
        };
        // Dispatch callback asynchronously
        DISPATCHER.send(info.clone());

        // Also write terminal deadlock record to the log if enabled
        if let Some(logger) = &self.logger {
            logger.log_deadlock(info);
        }
    }

    /// Handle a lock order violation detected via lock ordering analysis
    pub fn handle_lock_order_violation(
        &self,
        thread_id: ThreadId,
        lock_id: LockId,
        lock_cycle: Vec<LockId>,
    ) {
        let info = DeadlockInfo {
            thread_cycle: vec![thread_id],
            thread_waiting_for_locks: vec![(thread_id, lock_id)],
            lock_order_cycle: Some(lock_cycle),
            timestamp: Utc::now().to_rfc3339(),
        };
        DISPATCHER.send(info.clone());
        if let Some(logger) = &self.logger {
            logger.log_deadlock(info);
        }
    }
}
