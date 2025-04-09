use crate::core::types::{LockEvent, LockId, ThreadId};
use chrono::Utc;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Structure for a single log entry
#[derive(Debug, Serialize)]
pub struct LogEntry {
    /// Thread that performed the action
    pub thread_id: ThreadId,
    /// Lock that was involved
    pub lock_id: LockId,
    /// Type of event that occurred
    pub event: LockEvent,
    /// ISO 8601 timestamp of when the event occurred
    pub timestamp: String,
}

/// Determines how the logger should operate
#[derive(Debug)]
pub enum LoggerMode {
    /// Logging is disabled entirely
    Disabled,
    /// Log to the specified file
    ToFile(File),
}

/// Logger for recording lock events
pub struct Logger {
    mode: LoggerMode,
}

impl Logger {
    /// Create a new logger with logging disabled
    pub fn new() -> Self {
        Logger {
            mode: LoggerMode::Disabled,
        }
    }

    /// Create a new logger that writes to the specified file
    pub fn with_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Logger {
            mode: LoggerMode::ToFile(file),
        })
    }

    /// Log a lock event based on the configured mode
    pub fn log_event(&self, thread_id: ThreadId, lock_id: LockId, event: LockEvent) {
        // Early return if logging is disabled
        if let LoggerMode::Disabled = self.mode {
            return;
        }

        let entry = LogEntry {
            thread_id,
            lock_id,
            event,
            timestamp: Utc::now().to_rfc3339(),
        };

        if let LoggerMode::ToFile(ref file) = self.mode {
            let mut file = file;
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = writeln!(file, "{}", json);
                let _ = file.flush();
            }
        }
    }

    /// Check if logging is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self.mode, LoggerMode::Disabled)
    }
}

// Global logger instance
lazy_static::lazy_static! {
    static ref GLOBAL_LOGGER: Mutex<Logger> = Mutex::new(Logger::new());
}

/// Set the global logger to use the specified file, or disable logging if None
pub fn init_logger<P: AsRef<Path>>(path: Option<P>) -> std::io::Result<()> {
    if let Ok(mut global) = GLOBAL_LOGGER.lock() {
        match path {
            Some(path) => {
                *global = Logger::with_file(path)?;
            }
            None => {
                *global = Logger::new(); // Disabled mode
            }
        }
    }
    Ok(())
}

/// Log an event to the global logger (if enabled)
pub fn log_event(thread_id: ThreadId, lock_id: LockId, event: LockEvent) {
    if let Ok(logger) = GLOBAL_LOGGER.lock() {
        logger.log_event(thread_id, lock_id, event);
    }
}

/// Check if the global logger is enabled
pub fn is_logging_enabled() -> bool {
    if let Ok(logger) = GLOBAL_LOGGER.lock() {
        logger.is_enabled()
    } else {
        false
    }
}
