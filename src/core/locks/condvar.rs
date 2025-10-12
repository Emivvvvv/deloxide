use crate::core::detector;
use crate::core::locks::{NEXT_LOCK_ID, mutex::MutexGuard};
use crate::core::types::{CondvarId, get_current_thread_id};
use parking_lot::Condvar as ParkingLotCondvar;
use std::ops::DerefMut;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// A wrapper around a condition variable that tracks operations for deadlock detection
///
/// The Condvar provides the same interface as a standard condition variable but adds
/// deadlock detection by tracking wait and notify operations. It's a drop-in replacement
/// for std::sync::Condvar that enables deadlock detection.
///
/// # Example
///
/// ```no_run
/// use deloxide::{Mutex, Condvar};
/// use std::sync::Arc;
/// use std::thread;
///
/// let pair = Arc::new((Mutex::new(false), Condvar::new()));
/// let pair2 = Arc::clone(&pair);
///
/// // Spawn a thread that waits for the condition
/// thread::spawn(move || {
///     let (lock, cvar) = &*pair2;
///     let mut started = lock.lock();
///     while !*started {
///         cvar.wait(&mut started);
///     }
/// });
///
/// // Signal the condition in the main thread
/// let (lock, cvar) = &*pair;
/// let mut started = lock.lock();
/// *started = true;
/// cvar.notify_one();
/// ```
pub struct Condvar {
    /// Unique identifier for this condition variable
    id: CondvarId,
    /// The wrapped parking_lot condition variable
    inner: ParkingLotCondvar,
}

impl Condvar {
    /// Create a new Condvar with an automatically assigned ID
    ///
    /// # Returns
    /// A new Condvar ready for use with deadlock detection
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::Condvar;
    ///
    /// let condvar = Condvar::new();
    /// ```
    pub fn new() -> Self {
        let id = NEXT_LOCK_ID.fetch_add(1, Ordering::SeqCst);

        // Register the condvar with the detector
        detector::condvar::on_condvar_create(id);

        Condvar {
            id,
            inner: ParkingLotCondvar::new(),
        }
    }

    /// Get the ID of this condition variable
    ///
    /// # Returns
    /// The unique identifier assigned to this condition variable
    pub fn id(&self) -> CondvarId {
        self.id
    }

    /// Wait on this condition variable, releasing the associated mutex and blocking
    /// until another thread notifies this condition variable
    ///
    /// This method will atomically unlock the mutex specified (represented by the guard)
    /// and block the current thread. This means that any calls to notify() which happen
    /// logically after the mutex is unlocked are candidates to wake this thread up.
    /// When this function call returns, the lock specified will have been re-acquired.
    ///
    /// # Arguments
    /// * `guard` - A mutable reference to a MutexGuard that will be atomically unlocked
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// // In a real application, you would use this in a loop:
    /// // let mut guard = lock.lock();
    /// // while !*guard {
    /// //     cvar.wait(&mut guard);
    /// // }
    /// ```
    pub fn wait<'a, T>(&self, guard: &mut MutexGuard<'a, T>) {
        let thread_id = get_current_thread_id();
        let mutex_id = guard.lock_id();

        // Report wait begin - this logs the condvar wait and simulates mutex release
        detector::condvar::on_wait_begin(thread_id, self.id, mutex_id);

        // Explicitly report mutex release since parking_lot will unlock it internally
        detector::mutex::on_mutex_release(thread_id, mutex_id);

        // Perform the actual wait operation
        self.inner.wait(guard.inner_guard());

        // Report wait end and mutex reacquisition
        detector::condvar::on_wait_end(thread_id, self.id, mutex_id);
        detector::mutex::on_mutex_acquired(thread_id, mutex_id);
    }

    /// Wait on this condition variable with a timeout
    ///
    /// This method will atomically unlock the mutex specified (represented by the guard)
    /// and block the current thread. The thread will be blocked until another thread
    /// notifies this condition variable or until the timeout elapses. When this function
    /// returns, the lock specified will have been re-acquired.
    ///
    /// # Arguments
    /// * `guard` - A mutable reference to a MutexGuard that will be atomically unlocked
    /// * `timeout` - The maximum duration to wait
    ///
    /// # Returns
    /// `true` if the timeout elapsed, `false` if the condition variable was notified
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// let mut guard = lock.lock();
    /// let timed_out = cvar.wait_timeout(&mut guard, Duration::from_millis(100));
    /// if timed_out {
    ///     println!("Timed out waiting for condition");
    /// }
    /// ```
    pub fn wait_timeout<'a, T>(&self, guard: &mut MutexGuard<'a, T>, timeout: Duration) -> bool {
        let thread_id = get_current_thread_id();
        let mutex_id = guard.lock_id();

        // Report wait begin - this logs the condvar wait and simulates mutex release
        detector::condvar::on_wait_begin(thread_id, self.id, mutex_id);

        // Explicitly report mutex release since parking_lot will unlock it internally
        detector::mutex::on_mutex_release(thread_id, mutex_id);

        // Perform the actual wait operation with timeout
        let wait_result = self.inner.wait_for(guard.inner_guard(), timeout);
        let timed_out = wait_result.timed_out();

        // Report wait end and mutex reacquisition
        detector::condvar::on_wait_end(thread_id, self.id, mutex_id);
        detector::mutex::on_mutex_acquired(thread_id, mutex_id);

        timed_out
    }

    /// Blocks the current thread until the provided condition becomes false
    ///
    /// This is a convenience method that repeatedly calls `wait` while the condition
    /// returns true. It's equivalent to a while loop with wait.
    ///
    /// # Arguments
    /// * `guard` - A mutable reference to a MutexGuard
    /// * `condition` - A closure that returns true while waiting should continue
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    ///
    /// let pair = Arc::new((Mutex::new(true), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// let mut guard = lock.lock();
    /// // Wait while the value is true
    /// cvar.wait_while(&mut guard, |pending| *pending);
    /// ```
    pub fn wait_while<'a, T, F>(&self, guard: &mut MutexGuard<'a, T>, mut condition: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(guard.deref_mut()) {
            self.wait(guard);
        }
    }

    /// Waits on this condition variable with a timeout while a condition is true
    ///
    /// This is a convenience method that waits with a timeout while the condition
    /// returns true.
    ///
    /// # Arguments
    /// * `guard` - A mutable reference to a MutexGuard
    /// * `timeout` - The maximum duration to wait
    /// * `condition` - A closure that returns true while waiting should continue
    ///
    /// # Returns
    /// `true` if the timeout elapsed, `false` if the condition became false
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// let pair = Arc::new((Mutex::new(true), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// let mut guard = lock.lock();
    /// let timed_out = cvar.wait_timeout_while(
    ///     &mut guard,
    ///     Duration::from_millis(100),
    ///     |pending| *pending
    /// );
    /// ```
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        guard: &mut MutexGuard<'a, T>,
        timeout: Duration,
        mut condition: F,
    ) -> bool
    where
        F: FnMut(&mut T) -> bool,
    {
        let start = std::time::Instant::now();
        while condition(guard.deref_mut()) {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return true; // Timed out
            }
            let remaining = timeout - elapsed;
            if self.wait_timeout(guard, remaining) {
                return true; // Timed out in wait_timeout
            }
        }
        false // Condition became false
    }

    /// Wake up one blocked thread on this condition variable
    ///
    /// If there is a blocked thread on this condition variable, then it will be woken up
    /// from its call to wait or wait_timeout. Calls to notify_one are not buffered in any way.
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// // ... some other thread is waiting on cvar ...
    ///
    /// let mut guard = lock.lock();
    /// *guard = true;
    /// drop(guard); // Release the lock before notifying
    /// cvar.notify_one();
    /// ```
    pub fn notify_one(&self) {
        let thread_id = get_current_thread_id();

        // Report the notify operation to the detector first (for synthetic mutex attempts)
        detector::condvar::on_notify_one(self.id, thread_id);

        // Perform the actual notification
        self.inner.notify_one();
    }

    /// Wake up all blocked threads on this condition variable
    ///
    /// All threads currently waiting on this condition variable will be woken up from
    /// their call to wait or wait_timeout. Calls to notify_all are not buffered in any way.
    ///
    /// # Example
    ///
    /// ```rust
    /// use deloxide::{Mutex, Condvar};
    /// use std::sync::Arc;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let (lock, cvar) = &*pair;
    ///
    /// // ... multiple threads are waiting on cvar ...
    ///
    /// let mut guard = lock.lock();
    /// *guard = true;
    /// drop(guard); // Release the lock before notifying
    /// cvar.notify_all();
    /// ```
    pub fn notify_all(&self) {
        let thread_id = get_current_thread_id();

        // Report the notify operation to the detector first (for synthetic mutex attempts)
        detector::condvar::on_notify_all(self.id, thread_id);

        // Perform the actual notification
        self.inner.notify_all();
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Condvar {
    fn drop(&mut self) {
        // Register the condvar destruction with the detector
        detector::condvar::on_condvar_destroy(self.id);
    }
}
