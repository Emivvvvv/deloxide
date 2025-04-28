#![feature(thread_id_value)]
//! # Deloxide
//!
//! A cross-language deadlock detector with visualization support.
//!
//! Deloxide provides tools for detecting potential deadlocks in multithreaded
//! applications by tracking lock acquisitions and releases, and visualizing the
//! thread-lock relationships.
//!
//! ## Overview
//!
//! Deadlocks are a common concurrency issue that can be difficult to debug and reproduce.
//! Deloxide helps by tracking mutex interactions between threads and detecting potential
//! deadlock scenarios in real-time before they cause your application to hang.
//!
//! ## Features
//!
//! - **Real-time deadlock detection**: Monitors thread-lock interactions to detect deadlocks as they happen
//! - **Lock operation logging**: Records all lock operations for later analysis
//! - **Web-based visualization**: Visualize thread-lock relationships to understand deadlock patterns
//! - **Cross-language support**: Core implementation in Rust with C FFI bindings
//! - **Custom deadlock callbacks**: Execute custom actions when deadlocks are detected
//!
//! ## Usage Example
//!
//! ```rust
//! use deloxide::{Deloxide, TrackedMutex, TrackedThread};
//! use std::sync::Arc;
//! use std::time::Duration;
//! use std::thread;
//!
//! // Initialize the detector with a deadlock callback
//! Deloxide::new()
//!     .callback(|info| {
//!         println!("Deadlock detected! Cycle: {:?}", info.thread_cycle);
//!     })
//!     .start()
//!     .expect("Failed to initialize detector");
//!
//! // Create two mutexes
//! let mutex_a = Arc::new(TrackedMutex::new("Resource A"));
//! let mutex_b = Arc::new(TrackedMutex::new("Resource B"));
//!
//! // First thread: Lock A, then try to lock B
//! let a_clone = Arc::clone(&mutex_a);
//! let b_clone = Arc::clone(&mutex_b);
//! let t1 = TrackedThread::spawn(move || {
//!     let _lock_a = a_clone.lock().unwrap();
//!     thread::sleep(Duration::from_millis(100));
//!     let _lock_b = b_clone.lock().unwrap();
//! });
//!
//! // Second thread: Lock B, then try to lock A (potential deadlock)
//! let a_clone = Arc::clone(&mutex_a);
//! let b_clone = Arc::clone(&mutex_b);
//! let t2 = TrackedThread::spawn(move || {
//!     let _lock_b = b_clone.lock().unwrap();
//!     thread::sleep(Duration::from_millis(100));
//!     let _lock_a = a_clone.lock().unwrap();
//! });
//! ```

mod core;
pub use core::{
    DeadlockInfo, Deloxide, TrackedMutex, TrackedThread,
    types::{LockId, ThreadId},
};

mod showcase;
pub use showcase::{process_log_for_url, showcase, showcase_this};

pub mod ffi;

const BANNER: &str = r#"
      ▄ ▄▖▖ ▄▖▖▖▄▖▄ ▄▖      ▄▖  ▗   ▄▖
      ▌▌▙▖▌ ▌▌▚▘▐ ▌▌▙▖  ▌▌  ▛▌  ▜   ▛▌
      ▙▘▙▖▙▖▙▌▌▌▟▖▙▘▙▖  ▚▘▗ █▌▗ ▟▖▗ █▌
"#;
