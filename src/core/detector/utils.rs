use crate::core::Detector;
use crate::core::detector::DISPATCHER;
use crate::{DeadlockInfo, ThreadId};
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
            timestamp: Utc::now().to_rfc3339(),
        };
        // Dispatch callback asynchronously
        DISPATCHER.send(info.clone());

        // Also write terminal deadlock record to the log if enabled
        if let Some(logger) = &self.logger {
            logger.log_deadlock(info);
        }
    }
}
