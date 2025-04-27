//! Logging functionality for Deloxide
//!
//! This module provides logging capabilities for lock events and thread-lock
//! relationships, supporting deadlock detection and visualization.

mod event_logger;
mod graph_logger;

// Re-export core unified logging functionality
pub use event_logger::{
    get_current_log_file, init_logger, is_logging_enabled, log_interaction_event, log_lock_event,
    log_thread_event,
};
