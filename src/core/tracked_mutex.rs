use crate::core::detector;
use crate::core::types::{LockId, ThreadId, get_current_thread_id};
use parking_lot::{Mutex, MutexGuard};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};

// Global counter for generating unique lock IDs
static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

/// A wrapper around a mutex that tracks lock operations for deadlock detection
///
/// The TrackedMutex provides the same interface as a standard mutex, but adds
/// deadlock detection by tracking lock acquisition and release operations. It's
/// a drop-in replacement for std::sync::Mutex that enables deadlock detection.
///
/// # Example
///
/// ```rust
/// use deloxide::TrackedMutex;
/// use std::sync::Arc;
/// use std::thread;
///
/// // Initialize detector (not shown here)
///
/// // Create a tracked mutex
/// let mutex = Arc::new(TrackedMutex::new(42));
/// let mutex_clone = Arc::clone(&mutex);
///
/// // Use it just like a regular mutex
/// thread::spawn(move || {
///     let mut data = mutex.lock().unwrap();
///     *data += 1;
/// });
///
/// // In another thread
/// let mut data = mutex_clone.lock().unwrap();
/// *data += 10;
/// ```
pub struct TrackedMutex<T> {
    /// Unique identifier for this mutex
    id: LockId,
    /// The wrapped mutex
    inner: Mutex<T>,
    /// Thread that created this mutex
    creator_thread_id: ThreadId,
}

/// Guard for a TrackedMutex, reports lock release when dropped
///
/// The TrackedGuard provides the same interface as a standard mutex guard, but
/// additionally reports lock release to the deadlock detector when dropped. This
/// ensures that the detector's state is kept up-to-date with actual lock states.
pub struct TrackedGuard<'a, T> {
    /// Thread that owns this guard
    thread_id: ThreadId,
    /// Lock that this guard is for
    lock_id: LockId,
    /// The inner MutexGuard
    guard: MutexGuard<'a, T>,
}

impl<T> TrackedMutex<T> {
    /// Create a new TrackedMutex with an automatically assigned ID
    ///
    /// # Arguments
    /// * `value` - The initial value to store in the mutex
    ///
    /// # Returns
    /// A new TrackedMutex containing the provided value
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::TrackedMutex;
    ///
    /// let mutex = TrackedMutex::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        let id = NEXT_LOCK_ID.fetch_add(1, Ordering::SeqCst);
        let creator_thread_id = get_current_thread_id();

        // Register the lock with the detector, including creator thread info
        detector::on_lock_create(id, Some(creator_thread_id));

        TrackedMutex {
            id,
            inner: Mutex::new(value),
            creator_thread_id,
        }
    }

    /// Get the ID of this mutex
    ///
    /// # Returns
    /// The unique identifier assigned to this mutex
    pub fn id(&self) -> LockId {
        self.id
    }

    /// Get the ID of the thread that created this mutex
    ///
    /// # Returns
    /// The thread ID of the creator thread
    pub fn creator_thread_id(&self) -> ThreadId {
        self.creator_thread_id
    }

    /// Attempt to acquire the lock, tracking the attempt for deadlock detection
    ///
    /// This method records the lock attempt with the deadlock detector before
    /// trying to acquire the lock. If a deadlock would occur, the detector can
    /// identify it before the lock is actually acquired.
    ///
    /// # Returns
    /// A Result containing a TrackedGuard if the lock was acquired successfully
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::TrackedMutex;
    ///
    /// let mutex = TrackedMutex::new(42);
    /// {
    ///     let guard = mutex.lock().unwrap();
    ///     assert_eq!(*guard, 42);
    /// } // lock is automatically released when guard goes out of scope
    /// ```
    pub fn lock(&self) -> Result<TrackedGuard<T>, std::sync::PoisonError<MutexGuard<T>>> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::on_lock_attempt(thread_id, self.id);

        let guard = self.inner.lock();

        detector::on_lock_acquired(thread_id, self.id);
        Ok(TrackedGuard {
            thread_id,
            lock_id: self.id,
            guard,
        })
    }

    /// Try to acquire the lock without blocking
    ///
    /// This method attempts to acquire the lock without blocking, similar to
    /// std::sync::Mutex::try_lock(). It records the attempt with the deadlock detector.
    ///
    /// # Returns
    /// Some(TrackedGuard) if the lock was acquired, None if the lock was already held
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::TrackedMutex;
    ///
    /// let mutex = TrackedMutex::new(42);
    ///
    /// // Non-blocking attempt to acquire the lock
    /// if let Some(guard) = mutex.try_lock() {
    ///     // Lock was acquired
    ///     assert_eq!(*guard, 42);
    /// } else {
    ///     // Lock was already held by another thread
    ///     println!("Lock already held by another thread");
    /// }
    /// ```
    pub fn try_lock(&self) -> Option<TrackedGuard<'_, T>> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::on_lock_attempt(thread_id, self.id);

        if let Some(guard) = self.inner.try_lock() {
            detector::on_lock_acquired(thread_id, self.id);
            Some(TrackedGuard {
                thread_id,
                lock_id: self.id,
                guard,
            })
        } else {
            None
        }
    }
}

impl<T> Drop for TrackedMutex<T> {
    fn drop(&mut self) {
        // Register the lock destruction with the detector
        detector::on_lock_destroy(self.id);
    }
}

impl<T> Deref for TrackedGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<T> DerefMut for TrackedGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<T> Drop for TrackedGuard<'_, T> {
    fn drop(&mut self) {
        // Report lock release
        detector::on_lock_release(self.thread_id, self.lock_id);
    }
}
