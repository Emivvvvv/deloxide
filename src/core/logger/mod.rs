//! Logging functionality for Deloxide
//!
//! This module provides logging capabilities for lock events and thread-lock
//! relationships, supporting deadlock detection and visualization.
//!
//! The module is structured to allow each EventLogger instance to maintain its
//! own graph state for independent tracking of thread-lock relationships.

mod event_logger;
mod graph_logger;

// Re-export core unified logging functionality
pub use event_logger::{
    EventLogger,
    get_current_log_file
};