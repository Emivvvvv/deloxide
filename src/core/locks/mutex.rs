use crate::core::detector;
use crate::core::locks::NEXT_LOCK_ID;
use crate::core::types::{LockId, ThreadId, get_current_thread_id};
use parking_lot::{Mutex as ParkingLotMutex, MutexGuard as ParkingLotMutexGuard};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

/// A wrapper around a mutex that tracks lock operations for deadlock detection
///
/// The Mutex provides the same interface as a standard mutex but adds
/// deadlock detection by tracking lock acquisition and release operations. It's
/// a drop-in replacement for std::sync::Mutex that enables deadlock detection.
///
/// # Example
///
/// ```rust
/// use deloxide::Mutex;
/// use std::sync::Arc;
/// use std::thread;
///
/// // Initialize detectors (not shown here)
///
/// // Create a tracked mutex
/// let mutex = Arc::new(Mutex::new(42));
/// let mutex_clone = Arc::clone(&mutex);
///
/// // Use it just like a regular mutex
/// thread::spawn(move || {
///     let mut data = mutex.lock();
///     *data += 1;
/// });
///
/// // In another thread
/// let mut data = mutex_clone.lock();
/// *data += 10;
/// ```
pub struct Mutex<T> {
    /// Unique identifier for this mutex
    id: LockId,
    /// The wrapped mutex
    inner: ParkingLotMutex<T>,
    /// Thread that created this mutex
    creator_thread_id: ThreadId,
}

/// Guard for a Mutex, reports lock release when dropped
///
/// The MutexGuard provides the same interface as a standard mutex guard, but
/// additionally reports lock release to the deadlock detector when dropped. This
/// ensures that the detector's state is kept up to date with actual lock states.
pub struct MutexGuard<'a, T> {
    /// Thread that owns this guard
    thread_id: ThreadId,
    /// Lock that this guard is for
    lock_id: LockId,
    /// The inner MutexGuard
    guard: ParkingLotMutexGuard<'a, T>,
}

impl<T> Mutex<T> {
    /// Create a new Mutex with an automatically assigned ID
    ///
    /// # Arguments
    /// * `value` - The initial value to store in the mutex
    ///
    /// # Returns
    /// A new Mutex containing the provided value
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Mutex;
    ///
    /// let mutex = Mutex::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        let id = NEXT_LOCK_ID.fetch_add(1, Ordering::SeqCst);
        let creator_thread_id = get_current_thread_id();

        // Register the lock with the detector, including creator thread info
        detector::mutex::on_mutex_create(id, Some(creator_thread_id));

        Mutex {
            id,
            inner: ParkingLotMutex::new(value),
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
    /// trying to acquire the lock. If a deadlock occurs, the detector can
    /// identify it before the lock is actually acquired.
    ///
    /// # Returns
    /// A Result containing a MutexGuard if the lock was acquired successfully
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Mutex;
    ///
    /// let mutex = Mutex::new(42);
    /// {
    ///     let guard = mutex.lock();
    ///     assert_eq!(*guard, 42);
    /// } // lock is automatically released when guard goes out of scope
    /// ```
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::mutex::on_mutex_attempt(thread_id, self.id);

        let guard = self.inner.lock();

        detector::mutex::on_mutex_acquired(thread_id, self.id);
        MutexGuard {
            thread_id,
            lock_id: self.id,
            guard,
        }
    }

    /// Try to acquire the lock without blocking
    ///
    /// This method attempts to acquire the lock without blocking, similar to
    /// std::sync::Mutex::try_lock(). It records the attempt with the deadlock detector.
    ///
    /// # Returns
    /// Some(MutexGuard) if the lock was acquired, None if the lock was already held
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Mutex;
    ///
    /// let mutex = Mutex::new(42);
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
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::mutex::on_mutex_attempt(thread_id, self.id);

        if let Some(guard) = self.inner.try_lock() {
            detector::mutex::on_mutex_acquired(thread_id, self.id);
            Some(MutexGuard {
                thread_id,
                lock_id: self.id,
                guard,
            })
        } else {
            None
        }
    }

    /// Consumes this mutex, returning the underlying data
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Mutex;
    ///
    /// let mutex = Mutex::new(42);
    /// let value = mutex.into_inner();
    /// assert_eq!(value, 42);
    /// ```
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        // We need to prevent Drop from running since we're manually extracting the value
        // First, manually drop the detector tracking
        detector::mutex::on_mutex_destroy(self.id);

        // Use ManuallyDrop to prevent the automatic Drop implementation
        let mutex = std::mem::ManuallyDrop::new(self);

        // Safety: We're taking ownership and preventing double-drop
        unsafe { std::ptr::read(&mutex.inner) }.into_inner()
    }

    /// Returns a mutable reference to the underlying data
    ///
    /// Since this call borrows the Mutex mutably, no actual locking needs to
    /// take place – the mutable borrow statically guarantees no locks exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut() = 10;
    /// assert_eq!(*mutex.lock(), 10);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Drop for Mutex<T> {
    fn drop(&mut self) {
        // Register the lock destruction with the detector
        detector::mutex::on_mutex_destroy(self.id);
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<'a, T> MutexGuard<'a, T> {
    /// Get the inner parking_lot MutexGuard for condvar operations
    ///
    /// This method is used internally by Condvar to access the underlying
    /// parking_lot guard for wait operations.
    pub(crate) fn inner_guard(&mut self) -> &mut ParkingLotMutexGuard<'a, T> {
        &mut self.guard
    }

    /// Get the lock ID associated with this guard
    ///
    /// Returns the unique identifier of the mutex this guard protects.
    pub(crate) fn lock_id(&self) -> LockId {
        self.lock_id
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // Report lock release
        detector::mutex::on_mutex_release(self.thread_id, self.lock_id);
    }
}

// Trait implementations for better compatibility with std

impl<T: Default> Default for Mutex<T> {
    /// Creates a `Mutex<T>`, with the Default value for T
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

impl<T> From<T> for Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use
    /// This is equivalent to Mutex::new
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}
