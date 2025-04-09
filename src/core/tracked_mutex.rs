use crate::core::detector;
use crate::core::types::{LockId, ThreadId};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;

// Global counter for generating unique lock IDs
static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

/// A wrapper around std::sync::Mutex that tracks lock operations for deadlock detection
pub struct TrackedMutex<T> {
    /// Unique identifier for this mutex
    id: LockId,
    /// The wrapped mutex
    inner: Mutex<T>,
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
        TrackedMutex {
            id,
            inner: Mutex::new(value),
        }
    }

    /// Get the ID of this mutex
    pub fn id(&self) -> LockId {
        self.id
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
}

impl<'a, T> Deref for TrackedGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for TrackedGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<'a, T> Drop for TrackedGuard<'a, T> {
    fn drop(&mut self) {
        // Report lock release
        detector::on_lock_release(self.thread_id, self.lock_id);
    }
}
/// Get a unique identifier for the current thread
fn get_current_thread_id() -> ThreadId {
    // Convert the ThreadId to a usize for our internal use
    // This is a bit of a hack but it works for our purposes
    let id = thread::current().id();

    let id_ptr = &id as *const _ as usize;
    id_ptr
}
