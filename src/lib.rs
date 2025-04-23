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

mod core;
pub use core::{
    DeadlockInfo, Deloxide, TrackedMutex,
    types::{LockId, ThreadId},
};

mod showcase;
pub use showcase::{showcase, showcase_this};

pub mod ffi;

const BANNER: &str = r#"
      ▄ ▄▖▖ ▄▖▖▖▄▖▄ ▄▖      ▄▖  ▗   ▄▖
      ▌▌▙▖▌ ▌▌▚▘▐ ▌▌▙▖  ▌▌  ▛▌  ▜   ▛▌
      ▙▘▙▖▙▖▙▌▌▌▟▖▙▘▙▖  ▚▘▗ █▌▗ ▟▖▗ █▌
"#;