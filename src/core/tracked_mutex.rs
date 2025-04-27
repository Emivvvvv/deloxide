use crate::core::detector;
use crate::core::types::{LockId, ThreadId};
use crate::core::utils::get_current_thread_id;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

// Global counter for generating unique lock IDs
static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

/// A wrapper around std::sync::Mutex that tracks lock operations for deadlock detection
pub struct TrackedMutex<T> {
    /// Unique identifier for this mutex
    id: LockId,
    /// The wrapped mutex
    inner: Mutex<T>,
    /// Thread that created this mutex
    creator_thread_id: ThreadId,
}

/// Guard for a TrackedMutex, reports lock release when dropped
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
    pub fn id(&self) -> LockId {
        self.id
    }

    /// Get the ID of the thread that created this mutex
    pub fn creator_thread_id(&self) -> ThreadId {
        self.creator_thread_id
    }

    /// Attempt to acquire the lock, tracking the attempt for deadlock detection
    pub fn lock(&self) -> Result<TrackedGuard<T>, std::sync::PoisonError<MutexGuard<T>>> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::on_lock_attempt(thread_id, self.id);

        // Try to acquire the lock
        match self.inner.lock() {
            Ok(guard) => {
                // Report successful acquisition
                detector::on_lock_acquired(thread_id, self.id);
                Ok(TrackedGuard {
                    thread_id,
                    lock_id: self.id,
                    guard,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Try to acquire the lock without blocking
    pub fn try_lock(&self) -> Result<TrackedGuard<T>, std::sync::TryLockError<MutexGuard<T>>> {
        let thread_id = get_current_thread_id();

        // Report lock attempt
        detector::on_lock_attempt(thread_id, self.id);

        match self.inner.try_lock() {
            Ok(guard) => {
                detector::on_lock_acquired(thread_id, self.id);
                Ok(TrackedGuard {
                    thread_id,
                    lock_id: self.id,
                    guard,
                })
            }
            Err(e) => Err(e),
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
