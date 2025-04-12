//! # Deloxide
//!
//! A cross-language deadlock detector with visualization support.
//!
//! Deloxide provides tools for detecting potential deadlocks in multithreaded
//! applications by tracking lock acquisitions and releases, and visualizing the
//! thread-lock relationships.
//!
//! ## Features
//!
//! - Deadlock detection in real-time
//! - Lock operation logging
//! - Web-based visualization
//! - Cross-language support through FFI
//! - Tracked mutex implementation for Rust

pub mod core;
pub mod ffi;
pub mod showcase;

// Re-export core functionality for convenience
pub use core::{DeadlockInfo, Deloxide, types::{LockId, ThreadId}};
pub use showcase::showcase;
