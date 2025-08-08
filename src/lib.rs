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
//! Deadlocks are a common concurrency issue that can be challenging to debug and reproduce.
//! Deloxide helps by tracking mutex and reader-writer lock interactions between threads
//! and detecting potential deadlock scenarios in real-time before they cause your application to hang.
//!
//! ## Features
//!
//! - **Real-time deadlock detection**: Monitors thread-lock interactions to detect deadlocks as they happen
//! - **Multiple sync primitives**: Supports `Mutex`, `RwLock`, and `Condvar` for comprehensive deadlock detection
//! - **Lock operation logging**: Records all lock operations for later analysis
//! - **Web-based visualization**: Visualize thread-lock relationships to understand deadlock patterns
//! - **Cross-language support**: Core implementation in Rust with C FFI bindings
//! - **Custom deadlock callbacks**: Execute custom actions when deadlocks are detected
//!
//! ## Usage Examples
//!
//! ### Mutex Example
//!
//! ```rust
//! use deloxide::{Deloxide, Mutex, Thread};
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
//! let mutex_a = Arc::new(Mutex::new("Resource A"));
//! let mutex_b = Arc::new(Mutex::new("Resource B"));
//!
//! // First thread: Lock A, then try to lock B
//! let a_clone = Arc::clone(&mutex_a);
//! let b_clone = Arc::clone(&mutex_b);
//! let t1 = Thread::spawn(move || {
//!     let lock_a = a_clone.lock();
//!     thread::sleep(Duration::from_millis(100));
//!     let lock_b = b_clone.lock();
//! });
//!
//! // Second thread: Lock B, then try to lock A (potential deadlock)
//! let a_clone = Arc::clone(&mutex_a);
//! let b_clone = Arc::clone(&mutex_b);
//! let t2 = Thread::spawn(move || {
//!     let lock_b = b_clone.lock();
//!     thread::sleep(Duration::from_millis(100));
//!     let lock_a = a_clone.lock();
//! });
//! ```
//!
//! ### RwLock Example
//!
//! ```rust
//! use deloxide::{Deloxide, RwLock, Thread};
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
//! // Create an RwLock
//! let rwlock = Arc::new(RwLock::new("Shared Resource"));
//!
//! // Multiple reader threads
//! for i in 0..3 {
//!     let rwlock_clone = Arc::clone(&rwlock);
//!     Thread::spawn(move || {
//!         let read_guard = rwlock_clone.read();
//!         println!("Reader {} acquired read lock", i);
//!         thread::sleep(Duration::from_millis(50));
//!         // Read lock is automatically released when guard is dropped
//!     });
//! }
//!
//! // Writer thread that tries to upgrade (potential deadlock with readers)
//! let rwlock_clone = Arc::clone(&rwlock);
//! Thread::spawn(move || {
//!     let read_guard = rwlock_clone.read();
//!     println!("Writer acquired read lock, attempting to upgrade...");
//!     thread::sleep(Duration::from_millis(25));
//!     let write_guard = rwlock_clone.write(); // This will deadlock!
//!     println!("Writer acquired write lock");
//! });
//! ```
//!
//! ### Condvar Example
//!
//! ```rust,no_run
//! use deloxide::{Deloxide, Mutex, Condvar, Thread};
//! use std::sync::Arc;
//! use std::time::Duration;
//! use std::thread;
//!
//! // Initialize the detector (omitted callback for brevity)
//! let _ = Deloxide::new().start();
//!
//! let pair = Arc::new((Mutex::new(false), Condvar::new()));
//! let pair2 = pair.clone();
//!
//! // Thread waiting on condition
//! Thread::spawn(move || {
//!     let (mutex, condvar) = (&pair2.0, &pair2.1);
//!     let mut ready = mutex.lock();
//!     while !*ready {
//!         condvar.wait(&mut ready);
//!     }
//! });
//!
//! // Notifier thread
//! let pair3 = pair.clone();
//! Thread::spawn(move || {
//!     thread::sleep(Duration::from_millis(50));
//!     let (mutex, condvar) = (&pair3.0, &pair3.1);
//!     let mut ready = mutex.lock();
//!     *ready = true;
//!     condvar.notify_one();
//! });
//!
//! thread::sleep(Duration::from_millis(150));
//! ```

mod core;
pub use core::{
    Condvar, DeadlockInfo, Deloxide, Mutex, RwLock, Thread,
    types::{LockId, ThreadId},
};

#[cfg(feature = "stress-test")]
pub use core::{StressConfig, StressMode};

mod showcase;
pub use showcase::{process_log_for_url, showcase, showcase_this};

pub mod ffi;

// Ascii art font name "miniwi"
const BANNER: &str = r#"
▄ ▄▖▖ ▄▖▖▖▄▖▄ ▄▖    ▄▖  ▄▖  ▄▖
▌▌▙▖▌ ▌▌▚▘▐ ▌▌▙▖  ▌▌▛▌  ▄▌  ▛▌▄▖▛▌▛▘█▌
▙▘▙▖▙▖▙▌▌▌▟▖▙▘▙▖  ▚▘█▌▗ ▙▖▗ █▌  ▙▌▌ ▙▖
                                ▌
"#;
