//! Native threads with deadlock detection.
//!
//! This module provides a drop-in replacement for `std::thread` with additional
//! deadlock detection capabilities. It re-exports all items from `std::thread`
//! and overrides key functions like `spawn` to include tracking.
//!
//! ## Usage
//!
//! ```rust
//! use deloxide::thread;
//!
//! let handle = thread::spawn(|| {
//!     println!("Hello from a tracked thread!");
//!     42
//! });
//!
//! let result = handle.join().unwrap();
//! assert_eq!(result, 42);
//!
//! // All std::thread functions are available
//! thread::yield_now();
//! thread::sleep(std::time::Duration::from_millis(100));
//! let current = thread::current();
//! ```

use crate::core::detector;
use crate::core::types::get_current_thread_id;

// Re-export all items from std::thread
pub use std::thread::{
    AccessError, JoinHandle, LocalKey, Result, Scope, ScopedJoinHandle, Thread, ThreadId,
    available_parallelism, current, panicking, park, park_timeout, sleep, yield_now,
};

/// Spawns a new thread with deadlock detection, returning a [`JoinHandle`] for it.
///
/// This function is a drop-in replacement for [`std::thread::spawn`] that adds
/// deadlock detection by tracking thread creation and termination. It records
/// the parent-child relationship between threads for proper resource tracking.
///
/// The join handle can be used to block on termination of the spawned thread.
///
/// # Panics
///
/// Panics if the OS fails to create a thread; use [`Builder::spawn`]
/// to recover from such errors.
///
/// # Examples
///
/// ```rust
/// use deloxide::thread;
///
/// let handle = thread::spawn(|| {
///     println!("Hello from a spawned thread!");
///     42
/// });
///
/// let result = handle.join().unwrap();
/// assert_eq!(result, 42);
/// ```
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).unwrap()
}

/// Thread factory, which can be used in order to configure the properties of a new thread.
///
/// This is a wrapper around [`std::thread::Builder`] that adds deadlock detection
/// to spawned threads. It provides the same interface as `std::thread::Builder`.
///
/// # Examples
///
/// ```rust
/// use deloxide::thread;
///
/// let builder = thread::Builder::new()
///     .name("my-thread".to_string())
///     .stack_size(32 * 1024);
///
/// let handle = builder.spawn(|| {
///     println!("Hello from configured thread!");
/// }).unwrap();
///
/// handle.join().unwrap();
/// ```
pub struct Builder {
    inner: std::thread::Builder,
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deloxide::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".to_string())
    ///     .stack_size(32 * 1024);
    ///
    /// let handle = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handle.join().unwrap();
    /// ```
    pub fn new() -> Builder {
        Builder {
            inner: std::thread::Builder::new(),
        }
    }

    /// Names the thread-to-be. Currently the name is used for identification
    /// only in panic messages.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deloxide::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".to_string());
    ///
    /// let handle = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"));
    /// }).unwrap();
    ///
    /// handle.join().unwrap();
    /// ```
    pub fn name(mut self, name: String) -> Builder {
        self.inner = self.inner.name(name);
        self
    }

    /// Sets the size of the stack (in bytes) for the new thread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deloxide::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .stack_size(32 * 1024);
    /// ```
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.inner = self.inner.stack_size(size);
        self
    }

    /// Spawns a new thread with deadlock detection by executing the provided
    /// closure on it, returning a [`JoinHandle`] for it.
    ///
    /// # Errors
    ///
    /// Returns an error if the thread could not be spawned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deloxide::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handle = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handle.join().unwrap();
    /// ```
    pub fn spawn<F, T>(self, f: F) -> std::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        // Get the current thread ID (which will be the parent of the new thread)
        let parent_tid = get_current_thread_id();

        self.inner.spawn(move || {
            let tid = get_current_thread_id();
            // Register thread spawn with parent information
            detector::thread::on_thread_spawn(tid, Some(parent_tid));

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

            // Register thread exit
            detector::thread::on_thread_exit(tid);

            match result {
                Ok(val) => val,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }

    /// Spawns a new scoped thread with deadlock detection by executing the provided
    /// closure on it, returning a [`ScopedJoinHandle`] for it.
    ///
    /// # Errors
    ///
    /// Returns an error if the thread could not be spawned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deloxide::thread;
    /// use std::sync::atomic::{AtomicI32, Ordering};
    ///
    /// let x = AtomicI32::new(0);
    ///
    /// thread::scope(|s| {
    ///     thread::Builder::new()
    ///         .spawn_scoped(s, || {
    ///             x.fetch_add(1, Ordering::SeqCst);
    ///         }).unwrap();
    ///
    ///     thread::Builder::new()
    ///         .spawn_scoped(s, || {
    ///             x.fetch_add(1, Ordering::SeqCst);
    ///         }).unwrap();
    /// });
    ///
    /// assert_eq!(x.load(Ordering::SeqCst), 2);
    /// ```
    pub fn spawn_scoped<'scope, 'env, F, T>(
        self,
        scope: &'scope Scope<'scope, 'env>,
        f: F,
    ) -> std::io::Result<ScopedJoinHandle<'scope, T>>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        // Get the current thread ID (which will be the parent of the new thread)
        let parent_tid = get_current_thread_id();

        self.inner.spawn_scoped(scope, move || {
            let tid = get_current_thread_id();
            // Register thread spawn with parent information
            detector::thread::on_thread_spawn(tid, Some(parent_tid));

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

            // Register thread exit
            detector::thread::on_thread_exit(tid);

            match result {
                Ok(val) => val,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a scope for spawning scoped threads with deadlock detection.
///
/// This is a wrapper around [`std::thread::scope`] that adds deadlock detection
/// to scoped threads.
///
/// # Examples
///
/// ```rust
/// use deloxide::thread;
/// use std::sync::atomic::{AtomicI32, Ordering};
///
/// let x = AtomicI32::new(0);
///
/// thread::scope(|s| {
///     s.spawn(|| {
///         x.fetch_add(1, Ordering::SeqCst);
///     });
///
///     s.spawn(|| {
///         x.fetch_add(1, Ordering::SeqCst);
///     });
/// });
///
/// assert_eq!(x.load(Ordering::SeqCst), 2);
/// ```
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    std::thread::scope(f)
}
