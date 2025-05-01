use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Thread identifier type
///
/// Uniquely identifies a thread in the application.
pub type ThreadId = usize;

// Global counter for assigning unique thread IDs
static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

// Thread-local storage for each thread's assigned ID
thread_local! {
    static THREAD_ID: ThreadId = {
        // Each thread gets a unique ID once, when this is first accessed
        THREAD_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
    };
}

/// Get a unique identifier of the current thread
/// This will always return the same ID for the lifetime of the thread
pub fn get_current_thread_id() -> ThreadId {
    THREAD_ID.with(|&id| id)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn test_thread_id_consistency() {
        let (tx, rx) = mpsc::channel();

        // Create a thread
        let handle = thread::spawn(move || {
            let id1 = get_current_thread_id();
            let id2 = get_current_thread_id();
            let id3 = get_current_thread_id();

            // All calls should return the same ID
            assert_eq!(id1, id2);
            assert_eq!(id2, id3);

            tx.send(id1).unwrap();
        });

        let thread_id = rx.recv().unwrap();
        handle.join().unwrap();

        // Verify the thread kept the same ID throughout its lifetime
        println!("Thread ID: {}", thread_id);
    }

    #[test]
    fn test_thread_id_uniqueness() {
        let (tx, rx) = mpsc::channel();

        // Create multiple threads
        let mut handles = vec![];
        for _ in 0..10 {
            let tx = tx.clone();
            handles.push(thread::spawn(move || {
                let id = get_current_thread_id();
                tx.send(id).unwrap();
            }));
        }

        // Collect all thread IDs
        let mut ids = vec![];
        for _ in 0..10 {
            ids.push(rx.recv().unwrap());
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all IDs are unique
        let mut unique_ids = ids.clone();
        unique_ids.sort();
        unique_ids.dedup();
        assert_eq!(ids.len(), unique_ids.len());
    }
}
