//! A tracked reader-writer lock for deadlock detection
//!
//! This RwLock provides the same interface as a standard reader-writer lock
//! but augments all lock/unlock operations with tracking for deadlock detection.
//! It is a drop-in replacement for std::sync::RwLock that enables advanced deadlock analysis.
//! # Example
//!
//! ```rust
//! use deloxide::RwLock;
//! use std::sync::Arc;
//! use std::thread;
//!
//! let lock = Arc::new(RwLock::new(10));
//! let lock_clone = Arc::clone(&lock);
//!
//! thread::spawn(move || {
//!     let data = lock_clone.read();
//!     println!("Read: {}", *data);
//! });
//!
//! let mut data = lock.write();
//! *data += 1;
//! ```

use crate::core::detector;
use crate::core::locks::NEXT_LOCK_ID;


use crate::core::types::{LockId, ThreadId, get_current_thread_id};
#[cfg(feature = "logging-and-visualization")]
use crate::core::{Events, logger};
use parking_lot::{
    RwLock as ParkingLotRwLock, RwLockReadGuard as ParkingLotReadGuard,
    RwLockWriteGuard as ParkingLotWriteGuard,
};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A wrapper around a reader-writer lock that tracks operations for deadlock detection
///
/// The RwLock provides the same API as a standard reader-writer lock
/// but also notifies the detector on lock/unlock operations.
///
pub struct RwLock<T> {
    /// Unique identifier for this lock
    id: LockId,
    /// The wrapped RwLock
    inner: ParkingLotRwLock<T>,
    /// Thread that created this lock
    creator_thread_id: ThreadId,
    /// Tracks the thread ID of a WRITER using AtomicUsize. 0 if no writer.
    writer_owner: AtomicUsize,
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
    /// Reference to the owner atomic to clear it on drop
    owner_atomic: &'a AtomicUsize,
    /// Whether this lock acquisition was tracked by the global detector
    tracked_globally: bool,
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
    /// ```rust
    /// use deloxide::RwLock;
    /// let lock = RwLock::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        let id = NEXT_LOCK_ID.fetch_add(1, Ordering::SeqCst);
        let creator_thread_id = get_current_thread_id();
        detector::rwlock::create_rwlock(id, Some(creator_thread_id));
        RwLock {
            id,
            inner: ParkingLotRwLock::new(value),
            creator_thread_id,
            writer_owner: AtomicUsize::new(0),
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
    /// Uses two-phase locking protocol to eliminate race conditions between
    /// deadlock detection and lock acquisition.
    ///
    /// # Returns
    /// A guard which releases the lock when dropped
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        let thread_id = get_current_thread_id();

        // Phase 1: Atomic detection and try-acquire
        let guard = crate::core::detector::rwlock::attempt_read(thread_id, self.id, || {
            self.inner.try_read()
        });

        // Phase 2: If try-acquire failed, use blocking read
        let guard = match guard {
            Some(g) => g,
            None => {
                let g = self.inner.read();
                detector::rwlock::complete_read(thread_id, self.id);
                g
            }
        };

        RwLockReadGuard {
            thread_id,
            lock_id: self.id,
            guard,
        }
    }

    /// Acquire an exclusive (write) lock, tracking the attempt and acquisition
    ///
    /// Uses two-phase locking protocol to eliminate race conditions between
    /// deadlock detection and lock acquisition.
    ///
    /// # Returns
    /// A guard which releases the lock when dropped
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        let thread_id = get_current_thread_id();
        let tid_usize = thread_id as usize;

        // Optimistic Fast Path (Writer) - Disabled during stress testing
        #[cfg(not(feature = "stress-test"))]
        if let Some(guard) = self.inner.try_write() {
            self.writer_owner.store(tid_usize, Ordering::Release);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::RwWriteAttempt);
                }
            }

            #[cfg(feature = "lock-order-graph")]
            detector::rwlock::complete_write(thread_id, self.id);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::RwWriteAcquired);
                }
            }

            return RwLockWriteGuard {
                thread_id,
                lock_id: self.id,
                guard,
                owner_atomic: &self.writer_owner,
                tracked_globally: cfg!(feature = "lock-order-graph"),
            };
        }

        // Slow Path
        // Check if a writer holds it locally
        let current_writer_val = self.writer_owner.load(Ordering::Acquire);
        let current_writer = if current_writer_val == 0 {
            None
        } else {
            Some(current_writer_val as ThreadId)
        };

        let deadlock_info =
            detector::rwlock::acquire_write_slow(thread_id, self.id, current_writer);

        if let Some(info) = deadlock_info {
            // Verify the edge is still valid (it might be stale if the writer released the lock).
            let is_stale = if let Some(expected_writer) = current_writer {
                let actual_writer = self.writer_owner.load(Ordering::Relaxed);
                !detector::deadlock_handling::verify_deadlock_edges(
                    &info,
                    thread_id,
                    self.id,
                    expected_writer,
                    actual_writer,
                )
            } else {
                false
            };

            if !is_stale {
                detector::deadlock_handling::process_deadlock(info);
            }
        }

        let guard = self.inner.write();

        detector::rwlock::complete_write(thread_id, self.id);
        self.writer_owner.store(tid_usize, Ordering::Release);

        RwLockWriteGuard {
            thread_id,
            lock_id: self.id,
            guard,
            owner_atomic: &self.writer_owner,
            tracked_globally: true,
        }
    }

    /// Try to acquire a shared (read) lock, tracking the attempt
    ///
    /// Uses atomic detection to ensure deadlock detection and acquisition
    /// happen together.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        let thread_id = get_current_thread_id();

        // Use atomic detection and try-acquire
        let guard = detector::rwlock::attempt_read(thread_id, self.id, || self.inner.try_read());

        guard.map(|g| RwLockReadGuard {
            thread_id,
            lock_id: self.id,
            guard: g,
        })
    }

    /// Try to acquire an exclusive (write) lock, tracking the attempt
    ///
    /// Uses atomic detection to ensure deadlock detection and acquisition
    /// happen together.
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        let thread_id = get_current_thread_id();

        if let Some(guard) = self.inner.try_write() {
            self.writer_owner
                .store(thread_id as usize, Ordering::Release);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::RwWriteAttempt);
                }
            }

            #[cfg(feature = "lock-order-graph")]
            detector::rwlock::complete_write(thread_id, self.id);

            #[cfg(feature = "logging-and-visualization")]
            {
                if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                    logger::log_interaction_event(thread_id, self.id, Events::RwWriteAcquired);
                }
            }

            Some(RwLockWriteGuard {
                thread_id,
                lock_id: self.id,
                guard,
                owner_atomic: &self.writer_owner,
                tracked_globally: cfg!(feature = "lock-order-graph"),
            })
        } else {
            None
        }
    }

    /// Consumes this RwLock, returning the underlying data
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::RwLock;
    ///
    /// let lock = RwLock::new(String::from("hello"));
    /// let s = lock.into_inner();
    /// assert_eq!(s, "hello");
    /// ```
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        // We need to prevent Drop from running since we're manually extracting the value
        // First, manually drop the detector tracking
        detector::rwlock::destroy_rwlock(self.id);

        // Use ManuallyDrop to prevent the automatic Drop implementation
        let rwlock = std::mem::ManuallyDrop::new(self);

        // Safety: We're taking ownership and preventing double-drop
        unsafe { std::ptr::read(&rwlock.inner) }.into_inner()
    }

    /// Returns a mutable reference to the underlying data
    ///
    /// Since this call borrows the RwLock mutably, no actual locking needs to
    /// take place – the mutable borrow statically guarantees no locks exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::RwLock;
    ///
    /// let mut lock = RwLock::new(0);
    /// *lock.get_mut() = 10;
    /// assert_eq!(*lock.read(), 10);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Drop for RwLock<T> {
    fn drop(&mut self) {
        detector::rwlock::destroy_rwlock(self.id);
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
        detector::rwlock::release_read(self.thread_id, self.lock_id);
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
        // 1. Clear local ownership
        self.owner_atomic.store(0, Ordering::Release);

        // 2. Report release (detector and/or logger)
        if self.tracked_globally {
            detector::rwlock::release_write(self.thread_id, self.lock_id);
        } else {
            #[cfg(feature = "logging-and-visualization")]
            if logger::LOGGING_ENABLED.load(Ordering::Relaxed) {
                logger::log_interaction_event(
                    self.thread_id,
                    self.lock_id,
                    Events::RwWriteReleased,
                );
            }
        }


    }
}

// Trait implementations for better compatibility with std

impl<T: Default> Default for RwLock<T> {
    /// Creates a new `RwLock<T>`, with the Default value for T
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

impl<T> From<T> for RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked
    /// This is equivalent to RwLock::new
    fn from(t: T) -> Self {
        RwLock::new(t)
    }
}
