use crate::core::detector;
use crate::core::locks::NEXT_LOCK_ID;

use crate::core::types::{LockId, ThreadId, get_current_thread_id};
#[cfg(feature = "logging-and-visualization")]
use crate::core::{Events, logger};
use parking_lot::{Mutex as ParkingLotMutex, MutexGuard as ParkingLotMutexGuard};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};

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
    /// Stores the ThreadId of the current owner (0 if unlocked).
    /// This allows us to skip the global detector on the fast path.
    owner: AtomicUsize,
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
    /// Reference to the owner atomic to clear it on drop
    owner_atomic: &'a AtomicUsize,
    /// Whether this lock acquisition was tracked by the global detector
    tracked_globally: bool,
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
        detector::mutex::create_mutex(id, Some(creator_thread_id));

        Mutex {
            id,
            inner: ParkingLotMutex::new(value),
            creator_thread_id,
            owner: AtomicUsize::new(0),
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

    /// Acquire the lock, blocking if necessary
    ///
    /// Uses atomic deadlock detection to prevent race conditions.
    ///
    /// Uses the Optimistic Fast Path: attempts to acquire the lock cheaply first.
    /// Only interacts with the global deadlock detector if the lock is contented.
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
        let tid_usize = thread_id;

        // Optimistic Fast Path (Disabled during stress testing to ensure full detector coverage)
        #[cfg(not(feature = "stress-test"))]
        if let Some(guard) = self.inner.try_lock() {
            self.owner.store(tid_usize, Ordering::Release);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAttempt);
                }
            }

            #[cfg(feature = "lock-order-graph")]
            detector::mutex::complete_acquire(thread_id, self.id);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAcquired);
                }
            }

            return MutexGuard {
                thread_id,
                lock_id: self.id,
                guard,
                owner_atomic: &self.owner,
                tracked_globally: cfg!(feature = "lock-order-graph"),
            };
        }

        // Slow Path (Contention)
        // Read the current owner to report the dependency.
        let mut current_owner_val = self.owner.load(Ordering::Acquire);

        // Adaptive Backoff:
        // If the lock is physically held but we don't see an owner yet, it means
        // the owner is in the tiny gap between acquiring the lock and setting the owner ID.
        if current_owner_val == 0 && self.inner.is_locked() {
            let mut spin_count = 0;
            while current_owner_val == 0 {
                if spin_count < 100 {
                    std::hint::spin_loop();
                } else {
                    std::thread::yield_now();
                }

                // Use Relaxed loading during the spin loop for performance
                current_owner_val = self.owner.load(Ordering::Relaxed);
                spin_count += 1;

                // Optimization: Only check lock state occasionally to reduce cache traffic
                // If the lock is released, current_owner_val might remain 0, so we must check.
                if spin_count % 16 == 0 && !self.inner.is_locked() {
                    break;
                }
            }
            // Final Acquire fence to ensure we see the data associated with the owner store
            std::sync::atomic::fence(Ordering::Acquire);
        }

        let current_owner = if current_owner_val == 0 {
            None
        } else {
            Some(current_owner_val as ThreadId)
        };

        let deadlock_info = detector::mutex::acquire_slow(thread_id, self.id, current_owner);

        if let Some(info) = deadlock_info {
            // Verify the edge is still valid (it might be stale if the owner released the lock).
            let is_stale = if let Some(expected_owner) = current_owner {
                let actual_owner = self.owner.load(Ordering::Relaxed);
                !detector::deadlock_handling::verify_deadlock_edges(
                    &info,
                    thread_id,
                    self.id,
                    expected_owner,
                    actual_owner,
                )
            } else {
                false
            };

            if !is_stale {
                detector::deadlock_handling::process_deadlock(info);
            }
        }

        // Block until we get the lock
        let guard = self.inner.lock();

        // Update state
        detector::mutex::complete_acquire(thread_id, self.id);
        self.owner.store(tid_usize, Ordering::Release);

        MutexGuard {
            thread_id,
            lock_id: self.id,
            guard,
            owner_atomic: &self.owner,
            tracked_globally: true,
        }
    }

    /// Try to acquire the lock without blocking
    ///
    /// Returns Some(guard) if successful, None if the lock is held.
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
        let tid_usize = thread_id;

        if let Some(guard) = self.inner.try_lock() {
            self.owner.store(tid_usize, Ordering::Release);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAttempt);
                }
            }

            #[cfg(feature = "lock-order-graph")]
            detector::mutex::complete_acquire(thread_id, self.id);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAcquired);
                }
            }

            Some(MutexGuard {
                thread_id,
                lock_id: self.id,
                guard,
                owner_atomic: &self.owner,
                tracked_globally: cfg!(feature = "lock-order-graph"),
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
        detector::mutex::destroy_mutex(self.id);

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
        detector::mutex::destroy_mutex(self.id);
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

    /// Clear local ownership tracking (used internally by Condvar)
    pub(crate) fn clear_ownership(&self) {
        self.owner_atomic.store(0, Ordering::Release);
    }

    /// Restore local ownership tracking (used internally by Condvar)
    pub(crate) fn restore_ownership(&self) {
        self.owner_atomic.store(self.thread_id, Ordering::Release);
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // 1. Clear local ownership first
        self.owner_atomic.store(0, Ordering::Release);

        // 2. Report lock release (detector and/or logger)
        if self.tracked_globally {
            detector::mutex::release_mutex(self.thread_id, self.lock_id);
        } else {
            #[cfg(feature = "logging-and-visualization")]
            if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                logger::log_interaction_event(self.thread_id, self.lock_id, Events::MutexReleased);
            }
        }
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
