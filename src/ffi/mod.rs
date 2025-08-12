/// FFI bindings for Deloxide C API
///
/// This module provides the C API bindings for the Deloxide deadlock detection library.
/// It maps C function calls to their Rust implementations, handling memory management,
/// thread tracking, and callback mechanisms to bridge the language boundary.
///
/// The FFI interface provides all the functionality needed to use Deloxide from C
/// including initialization, mutex tracking, thread tracking, and deadlock detection.
mod condvar;
mod core;
mod mutex;
mod rwlock;
mod showcase;
mod stress;
mod thread;

use crate::core::locks::mutex::MutexGuard;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::sync::atomic::AtomicBool;

// We'll keep each Rust guard alive here until the C code calls unlock.
thread_local! {
    // Each thread can hold multiple mutex guards at a time, keyed by the raw mutex pointer
    static FFI_GUARD: RefCell<HashMap<*mut c_void, MutexGuard<'static, ()>>> = RefCell::new(HashMap::new());
}

// Globals to track initialization state
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut DEADLOCK_DETECTED: AtomicBool = AtomicBool::new(false);
static IS_LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

// Optional callback function provided by C code
static mut DEADLOCK_CALLBACK: Option<extern "C" fn(*const c_char)> = None;

#[cfg(feature = "stress-test")]
use crate::StressConfig;
#[cfg(feature = "stress-test")]
use std::sync::atomic::AtomicU8;

#[cfg(feature = "stress-test")]
static STRESS_MODE: AtomicU8 = AtomicU8::new(0); // 0=None, 1=Random, 2=Component
#[cfg(feature = "stress-test")]
static mut STRESS_CONFIG: Option<StressConfig> = None;
