use serde::{Deserialize, Serialize};

/// Thread & Lock identifier types
pub type ThreadId = usize;
pub type LockId = usize;

/// Represents the type of thread/lock event that occurred
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Events {
    /// A new Thread/Lock is spawned
    Spawn,
    /// The Thread/Lock is exited/dropped
    Exit,
    /// Thread is attempting to acquire a lock
    Attempt,
    /// Thread successfully acquired a lock
    Acquired,
    /// Thread released a lock
    Released,
}

/// Represents the result of a deadlock detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockInfo {
    /// List of threads involved in the deadlock cycle
    pub thread_cycle: Vec<ThreadId>,
    /// Map of threads to locks they're waiting for
    pub thread_waiting_for_locks: Vec<(ThreadId, LockId)>,
    /// Timestamp when the deadlock was detected
    pub timestamp: String,
}
