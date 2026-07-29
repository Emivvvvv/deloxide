use crate::core::detector;
use crate::core::locks::{NEXT_LOCK_ID, contention::ContentionState};

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
    /// Owner and contention state used by the detector handshake.
    state: MutexState,
}

struct MutexState {
    /// Stores the ThreadId of the current owner (0 if unlocked).
    owner: AtomicUsize,
    /// Number of blocking slow-path operations that may depend on this lock.
    contention: ContentionState,
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
    /// Shared owner/contention state consulted on release.
    state: &'a MutexState,
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
            state: MutexState {
                owner: AtomicUsize::new(0),
                contention: ContentionState::new(),
            },
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
            self.state.owner.store(tid_usize, Ordering::Release);
            let tracked_globally =
                cfg!(feature = "lock-order-graph") || self.state.contention.has_waiters();

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAttempt);
                }
            }

            if tracked_globally {
                detector::mutex::complete_acquire(thread_id, self.id);
            }

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
                state: &self.state,
                tracked_globally,
            };
        }

        // Slow Path (Contention)
        let slow_waiter = self.state.contention.register();
        let (rechecked_guard, deadlock_info) = detector::mutex::acquire_slow_with_recheck(
            thread_id,
            self.id,
            || self.inner.try_lock(),
            || {
                let owner = self.state.owner.load(Ordering::Acquire);
                (owner != 0).then_some(owner as ThreadId)
            },
        );

        if let Some(info) = deadlock_info {
            detector::deadlock_handling::process_deadlock(info);
        }

        if let Some(guard) = rechecked_guard {
            self.state.owner.store(tid_usize, Ordering::Release);
            drop(slow_waiter);
            return MutexGuard {
                thread_id,
                lock_id: self.id,
                guard,
                state: &self.state,
                tracked_globally: true,
            };
        }

        let guard = self.inner.lock();
        self.state.owner.store(tid_usize, Ordering::Release);
        detector::mutex::complete_acquire(thread_id, self.id);
        drop(slow_waiter);

        MutexGuard {
            thread_id,
            lock_id: self.id,
            guard,
            state: &self.state,
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
            self.state.owner.store(tid_usize, Ordering::Release);
            let tracked_globally =
                cfg!(feature = "lock-order-graph") || self.state.contention.has_waiters();

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::MutexAttempt);
                }
            }

            if tracked_globally {
                detector::mutex::complete_acquire(thread_id, self.id);
            }

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
                state: &self.state,
                tracked_globally,
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
        self.state.owner.store(0, Ordering::Release);
    }

    /// Restore local ownership tracking (used internally by Condvar)
    pub(crate) fn restore_ownership(&self) {
        self.state.owner.store(self.thread_id, Ordering::Release);
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // 1. Clear local ownership first
        self.state.owner.store(0, Ordering::Release);

        // 2. Report lock release (detector and/or logger)
        if self.tracked_globally || self.state.contention.has_waiters() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    #[test]
    fn mutex_guard_keeps_one_tracking_reference() {
        let maximum_size = size_of::<ParkingLotMutexGuard<'static, ()>>() + 4 * size_of::<usize>();

        assert!(
            size_of::<MutexGuard<'static, ()>>() <= maximum_size,
            "guard stores more than one tracking reference"
        );
    }

    #[test]
    fn blocking_mutex_wait_is_visible_until_acquisition() {
        let lock = Arc::new(Mutex::new(()));
        let owner = lock.lock();
        let waiter_lock = Arc::clone(&lock);
        let (acquired_tx, acquired_rx) = mpsc::channel();

        let waiter = std::thread::spawn(move || {
            let _guard = waiter_lock.lock();
            acquired_tx.send(()).unwrap();
        });

        let deadline = Instant::now() + Duration::from_secs(1);
        while !lock.state.contention.has_waiters() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(lock.state.contention.has_waiters());

        drop(owner);
        acquired_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        waiter.join().unwrap();
        assert!(!lock.state.contention.has_waiters());
    }
}
