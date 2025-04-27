use crate::core::detector;
use crate::core::utils::get_current_thread_id;
use std::thread::{self, JoinHandle};

/// A wrapper around std::thread::JoinHandle that logs spawn and exit events
pub struct TrackedThread<T>(JoinHandle<T>);

impl<T> TrackedThread<T>
where
    T: Send + 'static,
{
    /// Spawn a new tracked thread.
    /// Logs a Spawn event when the thread begins, and an Exit event when it ends.
    /// Also tracks the parent-child relationship between threads.
    pub fn spawn<F>(f: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        // Get the current thread ID (which will be the parent of the new thread)
        let parent_tid = get_current_thread_id();

        let handle = thread::spawn(move || {
            let tid = get_current_thread_id();
            // Register thread spawn with parent information
            detector::on_thread_spawn(tid, Some(parent_tid));

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

            // Register thread exit
            detector::on_thread_exit(tid);

            match result {
                Ok(val) => val,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
        TrackedThread(handle)
    }

    /// Wait for the thread to finish and return its result.
    pub fn join(self) -> thread::Result<T> {
        self.0.join()
    }
}
