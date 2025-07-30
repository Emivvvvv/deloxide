use crate::core::detector;
use crate::core::types::get_current_thread_id;
use std::thread::{self, JoinHandle};

/// A wrapper around std::thread::JoinHandle that logs spawn and exit events
///
/// The Thread provides the same interface as a standard thread, but adds
/// deadlock detection by tracking thread creation and termination. It's a drop-in
/// replacement for std::thread that enables deadlock detection for threads.
///
/// When a Thread is spawned, it records the parent-child relationship between
/// threads, which is important for proper resource tracking and cleanup. When the
/// thread exits, it automatically notifies the deadlock detector.
///
/// # Example
///
/// ```rust
/// use deloxide::Thread;
/// use std::time::Duration;
/// use std::thread;
///
/// // Initialize detector (not shown here)
///
/// // Spawn a tracked thread
/// let handle = Thread::spawn(|| {
///     println!("Hello from a tracked thread!");
///     thread::sleep(Duration::from_millis(100));
///     42 // Return value
/// });
///
/// // Wait for the thread to complete
/// let result = handle.join().unwrap();
/// assert_eq!(result, 42);
/// ```
pub struct Thread<T>(JoinHandle<T>);

impl<T> Thread<T>
where
    T: Send + 'static,
{
    /// Spawn a new tracked thread.
    ///
    /// This method spawns a new thread and automatically tracks it with the deadlock
    /// detector. It logs a Spawn event when the thread begins, and an Exit event when
    /// it ends. The parent-child relationship between threads is also recorded.
    ///
    /// # Arguments
    /// * `f` - The function to run in the new thread
    ///
    /// # Returns
    /// A Thread handle that can be used to join the thread later
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Thread;
    ///
    /// let handle = Thread::spawn(|| {
    ///     // Thread code here
    ///     "Hello from tracked thread"
    /// });
    ///
    /// let result = handle.join().unwrap();
    /// assert_eq!(result, "Hello from tracked thread");
    /// ```
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
        Thread(handle)
    }

    /// Wait for the thread to finish and return its result.
    ///
    /// This method joins the thread, waiting for it to complete and returning its
    /// result. It has the same behavior as std::thread::JoinHandle::join().
    ///
    /// # Returns
    /// A Result containing the thread's return value, or an error if the thread panicked
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Thread;
    ///
    /// let handle = Thread::spawn(|| 42);
    /// let result = handle.join().unwrap();
    /// assert_eq!(result, 42);
    /// ```
    pub fn join(self) -> thread::Result<T> {
        self.0.join()
    }
}
