use crate::core::Detector;
use crate::core::detector::DISPATCHER;
use crate::{DeadlockInfo, LockId, ThreadId};
use chrono::Utc;

impl Detector {
    /// Filter a cycle by checking if all threads share a common lock
    ///
    /// This method implements a false-positive filter for cycle detection.
    /// If all threads in a detected cycle hold a common lock, they cannot
    /// actually deadlock because they would have to acquire that common lock
    /// in some order, which breaks the cycle.
    ///
    /// # Arguments
    /// * `cycle` - The detected cycle of thread IDs
    ///
    /// # Returns
    /// * Empty vector if all threads share a common lock (false positive)
    /// * The original cycle if no common lock exists (real deadlock)
    ///
    /// # Example
    /// ```text
    /// Thread A holds [Lock 1, Lock 2], waits for Lock 3
    /// Thread B holds [Lock 1, Lock 3], waits for Lock 2
    ///
    /// This looks like a cycle: A → B → A
    /// But both hold Lock 1, so they can't deadlock
    /// Returns: [] (false positive)
    /// ```
    pub fn filter_cycle_by_common_locks(&self, cycle: &[ThreadId]) -> Vec<ThreadId> {
        if cycle.is_empty() {
            return Vec::new();
        }

        // Get locks held by the first thread in the cycle
        let mut iter = cycle.iter();
        let first = *iter.next().unwrap();
        let mut intersection = self.thread_holds.get(&first).cloned().unwrap_or_default();

        // Find intersection with all other threads' held locks
        for &thread_id in iter {
            if let Some(holds) = self.thread_holds.get(&thread_id) {
                intersection = intersection.intersection(holds).copied().collect();
            } else {
                // Thread holds no locks, intersection is empty
                intersection.clear();
                break;
            }
        }

        // If intersection is empty, it's a real cycle (no common locks)
        // If intersection has locks, it's a false positive (threads share locks)
        if intersection.is_empty() {
            cycle.to_vec()
        } else {
            Vec::new()
        }
    }

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
