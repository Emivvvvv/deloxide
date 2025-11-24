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
//! use deloxide::{Deloxide, Mutex, thread};
//! use std::sync::Arc;
//! use std::time::Duration;
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
//! let t1 = thread::spawn(move || {
//!     let lock_a = a_clone.lock();
//!     thread::sleep(Duration::from_millis(100));
//!     let lock_b = b_clone.lock();
//! });
//!
//! // Second thread: Lock B, then try to lock A (potential deadlock)
//! let a_clone = Arc::clone(&mutex_a);
//! let b_clone = Arc::clone(&mutex_b);
//! let t2 = thread::spawn(move || {
//!     let lock_b = b_clone.lock();
//!     thread::sleep(Duration::from_millis(100));
//!     let lock_a = a_clone.lock();
//! });
//! ```
//!
//! ### RwLock Example
//!
//! ```rust
//! use deloxide::{Deloxide, RwLock, thread};
//! use std::sync::Arc;
//! use std::time::Duration;
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
//!     thread::spawn(move || {
//!         let read_guard = rwlock_clone.read();
//!         println!("Reader {} acquired read lock", i);
//!         thread::sleep(Duration::from_millis(50));
//!         // Read lock is automatically released when guard is dropped
//!     });
//! }
//!
//! // Writer thread that tries to upgrade (potential deadlock with readers)
//! let rwlock_clone = Arc::clone(&rwlock);
//! thread::spawn(move || {
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
//! ```rust
//! use deloxide::{Deloxide, Mutex, Condvar, thread};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! // Initialize the detector (omitted callback for brevity)
//! let _ = Deloxide::new().start();
//!
//! let pair = Arc::new((Mutex::new(false), Condvar::new()));
//! let pair2 = pair.clone();
//!
//! // Thread waiting on condition
//! thread::spawn(move || {
//!     let (mutex, condvar) = (&pair2.0, &pair2.1);
//!     let mut ready = mutex.lock();
//!     while !*ready {
//!         condvar.wait(&mut ready);
//!     }
//! });
//!
//! // Notifier thread
//! let pair3 = pair.clone();
//! thread::spawn(move || {
//!     thread::sleep(Duration::from_millis(50));
//!     let (mutex, condvar) = (&pair3.0, &pair3.1);
//!     let mut ready = mutex.lock();
//!     *ready = true;
//!     condvar.notify_one();
//! });
//!
//! thread::sleep(Duration::from_millis(150));
//! ```
//!
//! ## Visualization (Showcase)
//!
//! You can open the interactive visualization in your browser for a given log file,
//! or for the currently active log if you initialized logging with `with_log()`.
//!
//! ```rust,no_run
//! # #[cfg(feature = "logging-and-visualization")]
//! # {
//! use deloxide::{Deloxide, showcase, showcase_this};
//!
//! // Initialize with logging enabled (default is "deloxide.log")
//! Deloxide::new()
//!     .with_log("logs/deadlock_{timestamp}.json") // Optional: override default log path
//!     .callback(|info| {
//!         eprintln!("Deadlock: {:?}", info.thread_cycle);
//!         // Optionally open the current log automatically
//!         showcase_this().expect("Failed to launch visualization");
//!     })
//!     .start()
//!     .unwrap();
//!
//! // Or later, open a specific log file
//! showcase("logs/deadlock_20250101_120000.json").unwrap();
//! # }
//! # #[cfg(not(feature = "logging-and-visualization"))]
//! # {
//! #     // This example requires the `logging-and-visualization` feature.
//! # }
//! ```
//!
//! ## Lock Order Graph (optional feature)
//!
//! Enable the `lock-order-graph` feature to detect potential deadlocks by tracking
//! lock acquisition ordering patterns, even when threads don't actually block.
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! deloxide = { version = "0.4.0", features = ["lock-order-graph"] }
//! ```
//!
//! ```rust
//! #[cfg(feature = "lock-order-graph")]
//! {
//! use deloxide::Deloxide;
//!
//! // Enable lock order checking for development (enabled by default if feature is on)
//! Deloxide::new()
//!     // .no_lock_order_checking() // Optional: disable if needed
//!     .callback(|info| {
//!         use deloxide::DeadlockSource;
//!         match info.source {
//!             DeadlockSource::WaitForGraph => {
//!                 println!("🚨 ACTUAL DEADLOCK! Threads are blocked.");
//!             }
//!             DeadlockSource::LockOrderViolation => {
//!                 println!("⚠️  SUSPECTED DEADLOCK! Dangerous lock ordering pattern.");
//!             }
//!         }
//!     })
//!     .start()
//!     .unwrap();
//! }
//! ```
//!
//! ## Stress Testing (optional feature)
//!
//! Enable the `stress-test` feature to increase the probability of deadlocks by
//! strategically delaying threads before lock attempts.
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! deloxide = { version = "0.4.0", features = ["stress-test"] }
//! ```
//!
//! ```rust
//! #[cfg(feature = "stress-test")]
//! {
//! use deloxide::{Deloxide, StressConfig};
//!
//! // Random preemption strategy with default config
//! Deloxide::new()
//!     .with_random_stress()
//!     .start()
//!     .unwrap();
//!
//! // Component-based strategy with custom config
//! Deloxide::new()
//!     .with_component_stress()
//!     .with_stress_config(StressConfig {
//!         preemption_probability: 0.7,
//!         min_delay_us: 200,
//!         max_delay_us: 1500,
//!         preempt_after_release: true,
//!     })
//!     .start()
//!     .unwrap();
//! }
//! ```

mod core;
pub use core::{
    Deloxide,
    locks::condvar::Condvar,
    locks::mutex::{Mutex, MutexGuard},
    locks::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    thread,
    types::{DeadlockInfo, DeadlockSource, LockId, ThreadId},
};

#[cfg(feature = "stress-test")]
pub use core::{StressConfig, StressMode};

#[cfg(feature = "logging-and-visualization")]
mod showcase;
#[cfg(feature = "logging-and-visualization")]
pub use showcase::{showcase, showcase_this};

pub mod ffi;

// Ascii art font name "miniwi"
const BANNER: &str = r#"
▄ ▄▖▖ ▄▖▖▖▄▖▄ ▄▖    ▄▖  ▖▖  ▄▖
▌▌▙▖▌ ▌▌▚▘▐ ▌▌▙▖  ▌▌▛▌  ▙▌  ▛▌
▙▘▙▖▙▖▙▌▌▌▟▖▙▘▙▖  ▚▘█▌▗  ▌▗ █▌
"#;
