use crate::core::logger::graph_logger;
use crate::core::logger::graph_logger::GraphState;
use crate::core::types::{LockEvent, LockId, ThreadId};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Global logger instance and file name
lazy_static::lazy_static! {
    static ref GLOBAL_LOGGER: Mutex<EventLogger> = Mutex::new(EventLogger::new());
}

lazy_static::lazy_static! {
    static ref CURRENT_LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);
}

#[derive(Debug, Serialize)]
pub struct CombinedLogEntry {
    pub event: LogEntry,
    pub graph: GraphState,
}

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
    pub timestamp: f64,
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
pub struct EventLogger {
    mode: LoggerMode,
}

impl Default for EventLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogger {
    /// Create a new logger with logging disabled
    pub fn new() -> Self {
        EventLogger {
            mode: LoggerMode::Disabled,
        }
    }

    /// Create a new logger that writes to the specified file, adds timestamp if requested.
    pub fn with_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file_path: PathBuf;

        // Check if the directory exists
        if let Some(parent) = path_buf.parent() {
            if parent.to_string_lossy() != "" && !parent.exists() {
                std::fs::create_dir_all(parent)
                    .context(format!("Failed to create directory: {:?}", parent))?;
            }
        }

        // If the filename ends with timestamp placeholder, replace it
        let path_str = path_buf.to_string_lossy();
        if path_str.contains("{timestamp}") {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let new_path_str = path_str.replace("{timestamp}", &timestamp.to_string());
            file_path = PathBuf::from(new_path_str);
        } else {
            // Use the exact filename the user specified
            file_path = path_buf;
        }

        // Update the global registry
        if let Ok(mut current_path) = CURRENT_LOG_FILE.lock() {
            *current_path = Some(file_path.clone());
        }

        // Open the file in create/write mode
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .context(format!("Failed to create log file: {:?}", file_path))?;

        Ok(EventLogger {
            mode: LoggerMode::ToFile(file),
        })
    }

    /// Log a lock event based on the configured mode
    pub fn log_event(&self, thread_id: ThreadId, lock_id: LockId, event: LockEvent) {
        // Early return if logging is disabled
        if let LoggerMode::Disabled = self.mode {
            return;
        }

        // First update the graph state with this event
        graph_logger::update_graph(thread_id, lock_id, event);

        // Generate absolute timestamp as f64: seconds since Unix Epoch with microsecond precision
        let now = Utc::now();
        let timestamp = now.timestamp() as f64 + now.timestamp_subsec_micros() as f64 / 1_000_000.0;

        // Then create log entry
        let entry = LogEntry {
            thread_id,
            lock_id,
            event,
            timestamp,
        };

        // Get the updated graph state
        let graph = graph_logger::get_current_graph_state();

        // Create combined log entry
        let combined_entry = CombinedLogEntry {
            event: entry,
            graph,
        };

        if let LoggerMode::ToFile(ref file) = self.mode {
            let mut file = file;
            if let Ok(json) = serde_json::to_string(&combined_entry) {
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

/// Set the global logger to use the specified file, or disable logging if None
pub fn init_logger<P: AsRef<Path>>(path: Option<P>) -> Result<()> {
    if let Ok(mut global) = GLOBAL_LOGGER.lock() {
        match path {
            Some(path) => {
                *global =
                    EventLogger::with_file(path).context("Failed to create logger with file")?;
            }
            None => {
                *global = EventLogger::new(); // Disabled mode
            }
        }
    } else {
        anyhow::bail!("Failed to acquire lock on global logger");
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

/// Get current log file path
pub fn get_current_log_file() -> Option<PathBuf> {
    if let Ok(lock) = CURRENT_LOG_FILE.lock() {
        lock.clone()
    } else {
        None
    }
}
