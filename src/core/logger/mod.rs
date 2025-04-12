//! Logging functionality for Deloxide
//!
//! This module provides logging capabilities for lock events and thread-lock
//! relationships, supporting deadlock detection and visualization.

mod event_logger;
mod graph_logger;

// Re-export core functionality
pub use event_logger::{init_logger, is_logging_enabled, log_event};
