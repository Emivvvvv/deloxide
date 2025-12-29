use crate::DeadlockInfo;
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
            .filter_map(|&t| self.thread_waits_for.get(&t).map(|&l| (t, l)))
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

/// Verify if a reported deadlock is valid by checking current lock ownership
///
/// This function performs "Immediate Edge Verification" to filter out stale edges
/// that can occur when using atomic hints for Fast Path detection.
///
/// # Arguments
/// * `info` - The detected deadlock info
/// * `thread_id` - The ID of the current thread (the one verifying)
/// * `lock_id` - The ID of the lock the current thread is trying to acquire
/// * `expected_owner` - The thread ID that was expected to hold the lock (from atomic hint)
/// * `actual_owner` - The actual current owner of the lock (from atomic load)
///
/// # Returns
/// * `true` if the deadlock is valid (edges confirmed)
/// * `false` if the deadlock is stale (edges invalid)
pub fn verify_deadlock_edges(
    info: &DeadlockInfo,
    thread_id: ThreadId,
    lock_id: LockId,
    expected_owner: ThreadId,
    actual_owner: usize,
) -> bool {
    // Verify Outgoing Edge: Check if we are waiting for this specific lock
    let waiting_for_this = info
        .thread_waiting_for_locks
        .iter()
        .any(|&(t, l)| t == thread_id && l == lock_id);

    if !waiting_for_this {
        return false;
    }

    // Verify the expected owner still holds the lock
    if actual_owner != expected_owner {
        return false; // Stale outgoing edge
    }

    // Verify Incoming Edges: Ensure cycle consistency with our current state
    // If the cycle implies WE hold this lock, but we know we don't, then the cycle is stale.
    // NOTE: LockOrderViolation cycles are synthetic (just the current thread) and don't represent
    // a wait-for cycle, so we skip this check.
    if info.source == DeadlockSource::LockOrderViolation {
        return true;
    }

    // Find who is waiting for us in the cycle
    // The cycle is a list of threads [t1, t2, t3] where t1->t2->t3->t1
    let cycle_len = info.thread_cycle.len();
    let mut self_index = None;
    for (i, &t) in info.thread_cycle.iter().enumerate() {
        if t == thread_id {
            self_index = Some(i);
            break;
        }
    }

    if let Some(idx) = self_index {
        // The thread before us in the cycle is waiting for us
        let prev_idx = if idx == 0 { cycle_len - 1 } else { idx - 1 };
        let prev_thread = info.thread_cycle[prev_idx];

        // Check if prev_thread is waiting for THIS lock
        let prev_waiting_for_this = info
            .thread_waiting_for_locks
            .iter()
            .any(|&(t, l)| t == prev_thread && l == lock_id);

        if prev_waiting_for_this {
            // The cycle says prev_thread waits for US for THIS lock.
            // But we know WE don't hold it (actual_owner == expected_owner != us).
            // So this edge is stale.
            return false; // Stale incoming edge
        }
    }

    true
}
