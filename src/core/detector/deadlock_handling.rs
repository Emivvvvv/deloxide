use crate::DeadlockInfo;
#[cfg(feature = "lock-order-graph")]
use crate::LockId;
use crate::ThreadId;
use crate::core::detector::DISPATCHER;
use crate::core::logger;
use crate::core::{DeadlockSource, Detector};
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

        // A shared RwLock read can be held by every participant at once, so it
        // cannot prove that the observed cycle is an impossible snapshot.
        intersection.retain(|lock_id| !self.rwlock_readers.contains_key(lock_id));

        // If intersection is empty, it's a real cycle (no common locks)
        // If intersection has locks, it's a false positive (threads share locks)
        if intersection.is_empty() {
            cycle.to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn extract_deadlock_info(&self, cycle: Vec<ThreadId>) -> DeadlockInfo {
        // Optimization: Only include wait-for edges for threads in the cycle.
        // This reduces the size of the info struct and speeds up verification.
        let thread_waiting_for_locks = cycle
            .iter()
            .filter_map(|&t| {
                self.thread_waits_for
                    .get(&t)
                    .map(|intent| (t, intent.lock_id))
            })
            .collect();

        DeadlockInfo {
            source: DeadlockSource::WaitForGraph,
            thread_cycle: cycle,
            thread_waiting_for_locks,
            lock_order_cycle: None,
            timestamp: Utc::now().to_rfc3339(),
            verification_request: None,
        }
    }

    /// Handle a lock order violation detected via lock ordering analysis
    #[cfg(feature = "lock-order-graph")]
    pub fn extract_lock_order_violation_info(
        &self,
        thread_id: ThreadId,
        lock_id: LockId,
        lock_cycle: Vec<LockId>,
    ) -> DeadlockInfo {
        DeadlockInfo {
            source: DeadlockSource::LockOrderViolation,
            thread_cycle: vec![thread_id],
            thread_waiting_for_locks: vec![(thread_id, lock_id)],
            lock_order_cycle: Some(lock_cycle),
            timestamp: Utc::now().to_rfc3339(),
            verification_request: None,
        }
    }
}

/// Process a detected deadlock (log and dispatch callback)
///
/// This function should be called OUTSIDE the global detector lock
/// to avoid holding the lock while formatting messages or waiting for callbacks.
pub fn process_deadlock(info: DeadlockInfo) {
    // Dispatch callback asynchronously
    DISPATCHER.send(info.clone());

    // Also write terminal deadlock record to the log if enabled
    logger::log_deadlock(info);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_shared_read_lock_does_not_filter_cycle() {
        let mut detector = Detector::new();
        detector.thread_holds.entry(1).or_default().insert(99);
        detector.thread_holds.entry(2).or_default().insert(99);
        detector.rwlock_readers.entry(99).or_default().insert(1, 1);
        detector.rwlock_readers.entry(99).or_default().insert(2, 1);

        assert_eq!(detector.filter_cycle_by_common_locks(&[1, 2]), vec![1, 2]);
    }
}
