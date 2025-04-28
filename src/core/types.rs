use serde::{Deserialize, Serialize};

/// Thread identifier type
///
/// Uniquely identifies a thread in the application. This is typically
/// derived from the platform-specific thread ID but wrapped in our own type
/// for portability and potential future expansion.
pub type ThreadId = usize;

/// Lock identifier type
///
/// Uniquely identifies a mutex/lock in the application. Each TrackedMutex
/// is assigned a unique ID when created.
pub type LockId = usize;

/// Represents the type of thread/lock event that occurred
///
/// These events are used to track the lifecycle of threads and locks
/// and their interactions, which is essential for deadlock detection.
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
///
/// This structure contains detailed information about a detected deadlock,
/// including which threads are involved in the cycle and which locks they are
/// waiting for. This information is passed to the deadlock callback and can
/// be used to diagnose the root cause of the deadlock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockInfo {
    /// List of threads involved in the deadlock cycle
    ///
    /// This is the ordered list of threads that form a cycle in the wait-for graph.
    /// For example, if thread 1 is waiting for thread 2, and thread 2 is waiting for
    /// thread 1, the cycle would be [1, 2].
    pub thread_cycle: Vec<ThreadId>,

    /// Map of threads to locks they're waiting for
    ///
    /// This provides additional context about which specific locks each thread in
    /// the cycle is waiting to acquire. Each tuple is (thread_id, lock_id).
    pub thread_waiting_for_locks: Vec<(ThreadId, LockId)>,

    /// Timestamp when the deadlock was detected
    ///
    /// ISO-8601 formatted timestamp indicating when the deadlock was detected.
    pub timestamp: String,
}
