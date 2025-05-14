// deloxide-tests/src/lib.rs
//! Deloxide testing framework
//!
//! This crate provides testing utilities that can work with or without
//! the deadlock detector enabled, allowing for performance comparisons.

#[cfg(feature = "detector")]
use deloxide::{TrackedMutex, TrackedThread, Deloxide, DeadlockInfo};
#[cfg(not(feature = "detector"))]
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::ops::{Deref, DerefMut};

/// Wrapper type that conditionally uses TrackedMutex or regular Mutex
pub struct MyMutex<T> {
    #[cfg(feature = "detector")]
    inner: TrackedMutex<T>,
    #[cfg(not(feature = "detector"))]
    inner: Mutex<T>,
}

impl<T> MyMutex<T> {
    pub fn new(value: T) -> Self {
        #[cfg(feature = "detector")]
        return MyMutex { inner: TrackedMutex::new(value) };

        #[cfg(not(feature = "detector"))]
        return MyMutex { inner: Mutex::new(value) };
    }

    #[cfg(feature = "detector")]
    pub fn lock(&self) -> impl Deref<Target = T> + DerefMut + '_ {
        self.inner.lock().unwrap()
    }

    #[cfg(not(feature = "detector"))]
    pub fn lock(&self) -> impl Deref<Target = T> + DerefMut + '_ {
        self.inner.lock()
    }
}

/// Wrapper for thread spawning
pub fn spawn_thread<F, T>(f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(feature = "detector")]
    {
        TrackedThread::spawn(f).0
    }

    #[cfg(not(feature = "detector"))]
    {
        thread::spawn(f)
    }
}

#[cfg(feature = "detector")]
pub fn maybe_start_detector(
    callback: impl Fn(DeadlockInfo) + Send + Sync + 'static,
) {
    let mut deloxide = Deloxide::new().callback(callback);

    #[cfg(feature = "detector-log")]
    {
        deloxide = deloxide.with_log("test_deadlock.log");
    }

    deloxide.start().expect("Deloxide failed to start");
}

#[cfg(not(feature = "detector"))]
pub fn maybe_start_detector<T>(
    _callback: impl Fn(T) + Send + Sync + 'static,
) {}


/// Helper function to create Arc'd mutexes easily
pub fn new_arc_mutex<T>(value: T) -> Arc<MyMutex<T>> {
    Arc::new(MyMutex::new(value))
}

