//! Logger for recording lock and thread operations for deadlock detection
//!
//! This module provides an efficient logging mechanism for tracking thread and lock operations,
//! including thread creation/exit and lock acquisition/release events. It supports asynchronous
//! file I/O with batching for improved performance, and ensures log files are properly flushed
//! before being processed for visualization.
//!
//! The logger only records events - graph state is reconstructed in the frontend for better performance.

use crate::core::types::{DeadlockInfo, Events, LockId, ThreadId};
#[cfg(feature = "logging-and-visualization")]
use anyhow::{Result, anyhow};

#[cfg(feature = "logging-and-visualization")]
use std::sync::atomic::AtomicBool;

/// Global flag used by fast-path checks to avoid touching logging machinery.
#[cfg(feature = "logging-and-visualization")]
pub static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "logging-and-visualization")]
mod enabled {
    use super::*;
    use chrono::Utc;
    use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
    use serde::Serialize;
    use std::fs::{File, OpenOptions};
    use std::io::{BufWriter, Write};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::Duration;

    const DEFAULT_LOG_PATH: &str = "deadlock_detection_{timestamp}.log";

    /// Structure for a single log entry representing a thread or lock event
    #[derive(Debug, Serialize, Clone)]
    pub struct LogEntry {
        /// Monotonic sequence number for deterministic ordering
        pub sequence: u64,
        /// Thread that performed the action (0 for lock-only events)
        pub thread_id: ThreadId,
        /// Lock that was involved (0 for thread-only events)
        pub lock_id: LockId,
        /// Type of event that occurred
        pub event: Events,
        /// Absolute timestamp of when the event occurred (seconds since Unix Epoch)
        pub timestamp: f64,
        /// Optional parent/creator thread ID (for spawn events)
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_id: Option<ThreadId>,
        /// Optional thread ID that was woken by condvar notify (for CondvarNotifyOne/All events)
        #[serde(skip_serializing_if = "Option::is_none")]
        pub woken_thread: Option<ThreadId>,
    }

    /// Commands for controlling the async logger thread
    #[derive(Debug, Clone)]
    pub enum LoggerCommand {
        /// Write a log entry to the file
        LogEntry(LogEntry),
        /// Write a terminal deadlock record
        Deadlock(DeadlockInfo),
        /// Flush all pending entries to disk and signal completion
        Flush(Sender<()>),
    }

    /// Event logger for recording lock and thread operations
    ///
    /// The EventLogger provides asynchronous file I/O with batching capabilities
    /// to minimize performance overhead and uses a background thread to handle file writes.
    pub struct EventLogger {
        /// Channel sender for async communication with logger thread
        sender: Sender<LoggerCommand>,
        /// Sequence number for log entries
        sequence: AtomicU64,
    }

    impl Default for EventLogger {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for EventLogger {
        fn drop(&mut self) {
            // Attempt to flush remaining logs when the logger is dropped
            // This is important to ensure logs aren't lost if the program exits
            if let Err(e) = self.flush() {
                eprintln!("Warning: Failed to flush logs during EventLogger drop: {e:?}");
            }
        }
    }

    impl EventLogger {
        /// Create a new logger that writes to the default log file
        pub fn new() -> Self {
            Self::with_file(DEFAULT_LOG_PATH).unwrap_or_else(|e| {
                eprintln!(
                    "Failed to create default logger: {e}. Falling back to simple file logger."
                );
                // If default log creation fails, create a simple logger with basic timestamp
                let fallback_path = format!(
                    "deadlock_detection_{}.log",
                    Utc::now().format("%Y%m%d_%H%M%S")
                );
                Self::with_simple_file(&fallback_path).expect("Failed to create fallback logger")
            })
        }

        /// Create a new logger that writes to a simple file without timestamp replacement
        fn with_simple_file<P: AsRef<Path>>(path: P) -> Result<Self> {
            let path_buf = path.as_ref().to_path_buf();

            // Create directory if needed
            if let Some(parent) = path_buf.parent()
                && parent.to_string_lossy() != ""
                && !parent.exists()
            {
                std::fs::create_dir_all(parent)?;
            }

            // Update the global registry
            CURRENT_LOG_FILE.lock().unwrap().replace(path_buf.clone());

            // Create async logger thread
            let (tx, rx) = unbounded::<LoggerCommand>();
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path_buf)?;

            // Spawn async writer thread
            thread::spawn(move || async_logger_thread(file, rx));

            Ok(EventLogger {
                sender: tx,
                sequence: AtomicU64::new(0),
            })
        }

        /// Create a new logger that writes to the specified file asynchronously
        ///
        /// This function sets up an asynchronous logging system with a background
        /// writer thread that handles file I/O operations. Log entries are sent
        /// to the writer thread via a channel for batched writing.
        ///
        /// # Arguments
        /// * `path` - Path to the log file. If the filename contains "{timestamp}",
        ///   it will be replaced with the current timestamp.
        ///
        /// # Returns
        /// A Result containing the configured EventLogger or an error if setup fails
        ///
        /// # Errors
        /// Returns an error if:
        /// - The directory containing the log file could not be created
        /// - The log file could not be opened for writing
        /// - The async logger thread could not be spawned
        pub fn with_file<P: AsRef<Path>>(path: P) -> Result<Self> {
            let path_buf = path.as_ref().to_path_buf();

            // Create directory if needed
            if let Some(parent) = path_buf.parent()
                && parent.to_string_lossy() != ""
                && !parent.exists()
            {
                std::fs::create_dir_all(parent)?;
            }

            // Replace timestamp placeholder if present
            #[allow(clippy::literal_string_with_formatting_args)]
            let file_path = if path_buf.to_string_lossy().contains("{timestamp}") {
                let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
                PathBuf::from(
                    path_buf
                        .to_string_lossy()
                        .replace("{timestamp}", &timestamp.to_string()),
                )
            } else {
                path_buf
            };

            // Update the global registry
            CURRENT_LOG_FILE.lock().unwrap().replace(file_path.clone());

            // Create async logger thread
            let (tx, rx) = unbounded::<LoggerCommand>();
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&file_path)?;

            // Spawn async writer thread
            thread::spawn(move || async_logger_thread(file, rx));

            Ok(EventLogger {
                sender: tx,
                sequence: AtomicU64::new(0),
            })
        }

        /// Log any event
        ///
        /// This method handles thread events, lock events, and lock-thread interactions
        /// by sending them to the async logger thread for processing. The operation is
        /// non-blocking and will not fail if the channel is full or closed.
        ///
        /// # Arguments
        /// * `thread_id` - ID of the thread involved in the event (0 for lock-only events)
        /// * `lock_id` - ID of the lock involved (0 for thread-only events)
        /// * `event` - Type of event that occurred
        /// * `parent_id` - Optional parent/creator thread ID (for spawn events)
        /// * `woken_thread` - Optional thread ID that was woken (for CondvarNotifyOne/All events)
        pub fn log_event(
            &self,
            thread_id: ThreadId,
            lock_id: LockId,
            event: Events,
            parent_id: Option<ThreadId>,
            woken_thread: Option<ThreadId>,
        ) {
            let now = Utc::now();
            let timestamp =
                now.timestamp() as f64 + now.timestamp_subsec_micros() as f64 / 1_000_000.0;

            let entry = LogEntry {
                sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
                thread_id,
                lock_id,
                event,
                timestamp,
                parent_id,
                woken_thread,
            };

            self.log_entry(entry);
        }

        /// Enqueue a pre-built log entry (used by the global logging facade)
        pub(crate) fn log_entry(&self, entry: LogEntry) {
            if let Err(e) = self.sender.send(LoggerCommand::LogEntry(entry)) {
                eprintln!("Failed to send log entry: {e:?}");
            }
        }

        /// Force flush all pending log entries to disk
        ///
        /// This method ensures all buffered log entries are written to disk and
        /// the file is properly synchronized. It blocks until the flush operation
        /// is complete.
        ///
        /// # Returns
        /// A Result that is Ok if the flush succeeded, or an error if it failed
        ///
        /// # Errors
        /// Returns an error if:
        /// - The flush request could not be sent to the async thread
        /// - The flush confirmation was not received
        pub fn flush(&self) -> Result<()> {
            let (flush_tx, flush_rx) = bounded(1);
            self.sender.send(LoggerCommand::Flush(flush_tx))?;

            match flush_rx.recv_timeout(Duration::from_secs(10)) {
                Ok(_) => Ok(()),
                Err(_) => Err(anyhow::anyhow!(
                    "Flush operation timed out or logger thread unavailable"
                )),
            }
        }

        /// Log a thread event to the logger
        ///
        /// # Arguments
        /// * `thread_id` - ID of the thread involved
        /// * `parent_id` - Optional ID of the thread that created this thread
        /// * `event` - Type of event (ThreadSpawn or ThreadExit)
        pub fn log_thread_event(
            &self,
            thread_id: ThreadId,
            parent_id: Option<ThreadId>,
            event: Events,
        ) {
            self.log_event(thread_id, 0, event, parent_id, None);
        }

        /// Log a lock event to the logger
        ///
        /// # Arguments
        /// * `lock_id` - ID of the lock involved
        /// * `creator_id` - ID of the thread that created this lock (for Spawn events)
        /// * `event` - Type of event (MutexSpawn/MutexExit, RwSpawn/RwExit, CondvarSpawn/CondvarExit)
        pub fn log_lock_event(&self, lock_id: LockId, creator_id: Option<ThreadId>, event: Events) {
            self.log_event(0, lock_id, event, creator_id, None);
        }

        /// Log a thread-lock interaction event to the logger
        ///
        /// # Arguments
        /// * `thread_id` - ID of the thread involved
        /// * `lock_id` - ID of the lock involved
        /// * `event` - Type of event (Attempt, Acquired, or Released)
        pub fn log_interaction_event(&self, thread_id: ThreadId, lock_id: LockId, event: Events) {
            self.log_event(thread_id, lock_id, event, None, None);
        }

        /// Log a condvar notify event with information about which thread was woken
        ///
        /// # Arguments
        /// * `notifier_thread_id` - ID of the thread that called notify
        /// * `condvar_id` - ID of the condvar
        /// * `event` - Type of event (CondvarNotifyOne or CondvarNotifyAll)
        /// * `woken_thread` - Optional ID of the thread that was woken (for NotifyOne)
        pub fn log_condvar_notify(
            &self,
            notifier_thread_id: ThreadId,
            condvar_id: LockId,
            event: Events,
            woken_thread: Option<ThreadId>,
        ) {
            self.log_event(notifier_thread_id, condvar_id, event, None, woken_thread);
        }

        /// Log a terminal deadlock record
        pub fn log_deadlock(&self, info: DeadlockInfo) {
            if let Err(e) = self.sender.send(LoggerCommand::Deadlock(info)) {
                eprintln!("Failed to send deadlock record: {e:?}");
            }
        }
    }

    /// Async logger thread that batches writes to improve performance
    ///
    /// This function runs in a dedicated thread and handles all file I/O operations.
    /// It receives log entries through a channel and writes them to disk in batches,
    /// reducing the overhead of frequent disk writes.
    ///
    /// # Arguments
    /// * `file` - The file to write log entries to
    /// * `rx` - Channel receiver for incoming logger commands
    /// * `flushing` - Atomic flag indicating flush status
    fn async_logger_thread(file: File, rx: Receiver<LoggerCommand>) {
        let mut writer = BufWriter::new(file);

        // Loop until the channel is closed
        while let Ok(cmd) = rx.recv() {
            match cmd {
                LoggerCommand::LogEntry(entry) => {
                    if let Ok(json) = serde_json::to_string(&entry)
                        && let Err(e) = writeln!(writer, "{json}")
                    {
                        eprintln!("Logger write error: {e:?}");
                    }
                }
                LoggerCommand::Deadlock(info) => {
                    // Wrap as a distinct terminal record
                    #[derive(serde::Serialize)]
                    struct DeadlockRecord<'a> {
                        deadlock: &'a DeadlockInfo,
                        timestamp: f64,
                    }
                    let now = chrono::Utc::now();
                    let ts =
                        now.timestamp() as f64 + now.timestamp_subsec_micros() as f64 / 1_000_000.0;
                    let record = DeadlockRecord {
                        deadlock: &info,
                        timestamp: ts,
                    };
                    if let Ok(json) = serde_json::to_string(&record)
                        && let Err(e) = writeln!(writer, "{json}").and_then(|_| writer.flush())
                    {
                        eprintln!("Logger write error (deadlock): {e:?}");
                    }
                }
                LoggerCommand::Flush(responder) => {
                    if let Err(e) = writer.flush() {
                        eprintln!("Logger flush error: {e:?}");
                    }
                    let _ = responder.send(());
                }
            }
        }

        // Channel closed - perform final flush before thread exits
        if let Err(e) = writer.flush() {
            eprintln!("Logger final flush error: {e:?}");
        }
    }

    // Global logger instance and configuration
    lazy_static::lazy_static! {
        static ref CURRENT_LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);
    }

    /// Get current log file path
    pub fn get_current_log_file() -> Option<PathBuf> {
        CURRENT_LOG_FILE
            .try_lock()
            .ok()
            .and_then(|lock| lock.clone())
    }

    /// Flush pending log data and return the current log file path
    ///
    /// This helper is primarily used by `showcase_this()` to ensure the log on disk
    /// is fully synchronized before attempting to encode it for visualization.
    /// It returns an error if flushing fails or if no log file is currently active.
    pub fn prepare_showcase_log_path() -> Result<PathBuf> {
        flush_logs()?;
        get_current_log_file().ok_or_else(|| anyhow!("No active log file found"))
    }

    // ==========================================================================================
    // Global Facade Implementation
    // ==========================================================================================

    static GLOBAL_LOGGER: OnceLock<EventLogger> = OnceLock::new();

    #[inline]
    fn with_logger<F>(f: F)
    where
        F: FnOnce(&EventLogger),
    {
        if let Some(logger) = GLOBAL_LOGGER.get() {
            f(logger);
        }
    }

    pub fn init_logger(logger: EventLogger) {
        if GLOBAL_LOGGER.set(logger).is_ok() {
            LOGGING_ENABLED.store(true, Ordering::Relaxed);
        }
    }

    pub fn log_thread_event(thread_id: ThreadId, parent_id: Option<ThreadId>, event: Events) {
        with_logger(|logger| {
            logger.log_thread_event(thread_id, parent_id, event);
        });
    }

    pub fn log_lock_event(lock_id: LockId, creator_id: Option<ThreadId>, event: Events) {
        with_logger(|logger| {
            logger.log_lock_event(lock_id, creator_id, event);
        });
    }

    pub fn log_interaction_event(thread_id: ThreadId, lock_id: LockId, event: Events) {
        with_logger(|logger| {
            logger.log_interaction_event(thread_id, lock_id, event);
        });
    }

    pub fn log_condvar_notify_event(
        thread_id: ThreadId,
        condvar_id: LockId,
        event: Events,
        woken_thread: Option<ThreadId>,
    ) {
        with_logger(|logger| {
            logger.log_condvar_notify(thread_id, condvar_id, event, woken_thread);
        });
    }

    pub fn log_deadlock(info: DeadlockInfo) {
        with_logger(|logger| logger.log_deadlock(info));
    }

    pub fn flush_logs() -> Result<()> {
        if let Some(logger) = GLOBAL_LOGGER.get() {
            logger.flush()
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_basic_logging() {
            let temp_dir = TempDir::new().unwrap();
            let log_path = temp_dir.path().join("basic.log");

            let logger = EventLogger::with_file(&log_path).unwrap();

            // Log some events
            logger.log_event(1, 0, Events::ThreadSpawn, None, None);
            logger.log_event(1, 10, Events::MutexAttempt, None, None);
            logger.log_event(1, 10, Events::MutexAcquired, None, None);
            logger.log_event(1, 10, Events::MutexReleased, None, None);
            logger.log_event(1, 0, Events::ThreadExit, None, None);

            // Flush to ensure writes complete
            logger.flush().unwrap();

            // Read back the log file
            let contents = std::fs::read_to_string(&log_path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();

            assert_eq!(lines.len(), 5);
            assert!(lines[0].contains("\"thread_id\":1"));
            assert!(lines[0].contains("\"event\":\"ThreadSpawn\""));
        }

        #[test]
        fn test_flush_idempotence() {
            let temp_dir = TempDir::new().unwrap();
            let log_path = temp_dir.path().join("flush_test.log");

            let logger = EventLogger::with_file(&log_path).unwrap();

            // Log some events
            for i in 0..10 {
                logger.log_event(i, 0, Events::ThreadSpawn, None, None);
            }

            // Multiple flushes should not cause issues
            logger.flush().unwrap();
            logger.flush().unwrap();
            logger.flush().unwrap();

            let contents = std::fs::read_to_string(&log_path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();
            assert_eq!(lines.len(), 10);
        }

        #[test]
        fn test_graph_state_per_instance() {
            let temp_dir = TempDir::new().unwrap();
            let log_path1 = temp_dir.path().join("log1.log");
            let log_path2 = temp_dir.path().join("log2.log");

            let logger1 = EventLogger::with_file(&log_path1).unwrap();
            let logger2 = EventLogger::with_file(&log_path2).unwrap();

            // Log different events to each logger
            logger1.log_thread_event(1, None, Events::ThreadSpawn);
            logger1.log_lock_event(10, Some(1), Events::MutexSpawn);

            logger2.log_thread_event(2, None, Events::ThreadSpawn);
            logger2.log_lock_event(20, Some(2), Events::MutexSpawn);

            // Flush both
            logger1.flush().unwrap();
            logger2.flush().unwrap();

            // Verify they have different graph states
            let content1 = std::fs::read_to_string(&log_path1).unwrap();
            let content2 = std::fs::read_to_string(&log_path2).unwrap();

            assert!(content1.contains("\"thread_id\":1"));
            assert!(content1.contains("\"lock_id\":10"));
            assert!(!content1.contains("\"thread_id\":2"));
            assert!(!content1.contains("\"lock_id\":20"));

            assert!(content2.contains("\"thread_id\":2"));
            assert!(content2.contains("\"lock_id\":20"));
            assert!(!content2.contains("\"thread_id\":1"));
            assert!(!content2.contains("\"lock_id\":10"));
        }

        #[test]
        fn test_logger_drop_flushes() {
            let temp_dir = TempDir::new().unwrap();
            let log_path = temp_dir.path().join("drop_test.log");

            {
                // Create logger in a scope so it gets dropped
                let logger = EventLogger::with_file(&log_path).unwrap();
                logger.log_event(1, 0, Events::ThreadSpawn, None, None);
                // Logger is dropped here, which should trigger flush
            }

            // Give the async thread a moment to finish
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Verify the log was written
            let contents = std::fs::read_to_string(&log_path).unwrap();
            assert!(!contents.is_empty());
            assert!(contents.contains("\"thread_id\":1"));
        }
    }
}

// ==========================================================================================
// No-op implementation when logging is disabled
// ==========================================================================================

#[cfg(not(feature = "logging-and-visualization"))]
mod disabled {
    use super::*;

    pub fn log_thread_event(_: ThreadId, _: Option<ThreadId>, _: Events) {}
    pub fn log_lock_event(_: LockId, _: Option<ThreadId>, _: Events) {}
    pub fn log_interaction_event(_: ThreadId, _: LockId, _: Events) {}
    pub fn log_condvar_notify_event(_: ThreadId, _: LockId, _: Events, _: Option<ThreadId>) {}
    pub fn log_deadlock(_: DeadlockInfo) {}
}

#[cfg(feature = "logging-and-visualization")]
pub use enabled::*;

#[cfg(not(feature = "logging-and-visualization"))]
pub use disabled::*;
