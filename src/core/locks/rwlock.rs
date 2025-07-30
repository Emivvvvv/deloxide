//! A tracked reader-writer lock for deadlock detection
//!
//! This RwLock provides the same interface as a standard reader-writer lock,
//! but augments all lock/unlock operations with tracking for deadlock detection.
//! It is a drop-in replacement for std::sync::RwLock that enables advanced deadlock analysis.
//!
//! # Example
//!
//! ```no_run
//! use deloxide::RwLock;
//! use std::sync::Arc;
//! use std::thread;
//!
//! let lock = Arc::new(RwLock::new(10));
//! let lock_clone = Arc::clone(&lock);
//!
//! thread::spawn(move || {
//!     let data = lock_clone.read().unwrap();
//!     println!("Read: {}", *data);
//! });
//!
//! let mut data = lock.write().unwrap();
//! *data += 1;
//! ```

use crate::core::detector;
use crate::core::locks::NEXT_LOCK_ID;
use crate::core::types::{LockId, ThreadId, get_current_thread_id};
use parking_lot::{
    RwLock as ParkingLotRwLock, RwLockReadGuard as ParkingLotReadGuard,
    RwLockWriteGuard as ParkingLotWriteGuard,
};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

/// A wrapper around a reader-writer lock that tracks operations for deadlock detection
///
/// The RwLock provides the same API as a standard reader-writer lock,
/// but also notifies the detector on lock/unlock operations.
///
pub struct RwLock<T> {
    /// Unique identifier for this lock
    id: LockId,
    /// The wrapped RwLock
    inner: ParkingLotRwLock<T>,
    /// Thread that created this lock
    creator_thread_id: ThreadId,
}

/// Guard for a shared (read) lock, reports release when dropped
pub struct RwLockReadGuard<'a, T> {
    thread_id: ThreadId,
    lock_id: LockId,
    guard: ParkingLotReadGuard<'a, T>,
}

/// Guard for an exclusive (write) lock, reports release when dropped
pub struct RwLockWriteGuard<'a, T> {
    thread_id: ThreadId,
    lock_id: LockId,
    guard: ParkingLotWriteGuard<'a, T>,
}

impl<T> RwLock<T> {
    /// Create a new tracked RwLock with a unique ID
    ///
    /// # Arguments
    /// * `value` - The initial value to store in the lock
    ///
    /// # Returns
    /// A new RwLock wrapping the provided value
    ///
    /// # Example
    ///
    /// ```no_run
    /// use deloxide::RwLock;
    /// let lock = RwLock::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        let id = NEXT_LOCK_ID.fetch_add(1, Ordering::SeqCst);
        let creator_thread_id = get_current_thread_id();
        detector::rwlock::on_rwlock_create(id, Some(creator_thread_id));
        RwLock {
            id,
            inner: ParkingLotRwLock::new(value),
            creator_thread_id,
        }
    }

    /// Get the unique ID of this lock
    pub fn id(&self) -> LockId {
        self.id
    }

    /// Get the creator thread ID
    pub fn creator_thread_id(&self) -> ThreadId {
        self.creator_thread_id
    }

    /// Acquire a shared (read) lock, tracking the attempt and acquisition
    ///
    /// # Returns
    /// A guard which releases the lock when dropped
    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>, ()> {
        let thread_id = get_current_thread_id();
        detector::rwlock::on_rw_read_attempt(thread_id, self.id);
        let guard = self.inner.read();
        detector::rwlock::on_rw_read_acquired(thread_id, self.id);
        Ok(RwLockReadGuard {
            thread_id,
            lock_id: self.id,
            guard,
        })
    }

    /// Acquire an exclusive (write) lock, tracking the attempt and acquisition
    ///
    /// # Returns
    /// A guard which releases the lock when dropped
    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>, ()> {
        let thread_id = get_current_thread_id();
        detector::rwlock::on_rw_write_attempt(thread_id, self.id);
        let guard = self.inner.write();
        detector::rwlock::on_rw_write_acquired(thread_id, self.id);
        Ok(RwLockWriteGuard {
            thread_id,
            lock_id: self.id,
            guard,
        })
    }

    /// Try to acquire a shared (read) lock, tracking the attempt
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        let thread_id = get_current_thread_id();
        detector::rwlock::on_rw_read_attempt(thread_id, self.id);
        if let Some(guard) = self.inner.try_read() {
            detector::rwlock::on_rw_read_acquired(thread_id, self.id);
            Some(RwLockReadGuard {
                thread_id,
                lock_id: self.id,
                guard,
            })
        } else {
            None
        }
    }

    /// Try to acquire an exclusive (write) lock, tracking the attempt
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        let thread_id = get_current_thread_id();
        detector::rwlock::on_rw_write_attempt(thread_id, self.id);
        if let Some(guard) = self.inner.try_write() {
            detector::rwlock::on_rw_write_acquired(thread_id, self.id);
            Some(RwLockWriteGuard {
                thread_id,
                lock_id: self.id,
                guard,
            })
        } else {
            None
        }
    }
}

impl<T> Drop for RwLock<T> {
    fn drop(&mut self) {
        detector::rwlock::on_rwlock_destroy(self.id);
    }
}

// --- Guard Implementations ---

impl<'a, T> Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}
impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        detector::rwlock::on_rw_read_release(self.thread_id, self.lock_id);
    }
}

impl<'a, T> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}
impl<'a, T> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}
impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        detector::rwlock::on_rw_write_release(self.thread_id, self.lock_id);
    }
}
