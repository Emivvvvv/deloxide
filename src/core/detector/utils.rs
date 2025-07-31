use crate::core::Detector;
use crate::core::detector::DISPATCHER;
use crate::{DeadlockInfo, ThreadId};
use chrono::Utc;

impl Detector {
    pub fn handle_detected_deadlock(&self, cycle: Vec<ThreadId>) {
        DISPATCHER.send(DeadlockInfo {
            thread_cycle: cycle.clone(),
            thread_waiting_for_locks: self
                .thread_waits_for
                .iter()
                .map(|(&t, &l)| (t, l))
                .collect(),
            timestamp: Utc::now().to_rfc3339(),
        });
    }
}
