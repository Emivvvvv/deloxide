pub mod encoder;
use encoder::process_log_for_url;

use crate::core::detector::flush_global_detector_logs;
use crate::core::logger::{self};
use anyhow::{Context, Result};
use std::path::Path;

/// Showcase the log data by sending it to the showcase server
///
/// This function processes a log file and opens a browser window to visualize
/// the thread-lock relationships recorded in the log. The visualization is
/// hosted on a web server and can help identify the patterns that led to a deadlock.
///
/// # Arguments
/// * `log_path` - Path to the log file.
///
/// # Returns
/// A Result that is Ok if the showcase succeeded, or an error if it failed.
///
/// # Errors
/// Returns an error if:
/// - Failed to read the log file
/// - Failed to process the log file
/// - Failed to open the browser
///
/// # Example
///
/// ```no_run
/// use deloxide::showcase;
/// use std::path::Path;
///
/// // After a deadlock has been detected and logged
/// let log_path = Path::new("deadlock_log.json");
/// showcase(log_path).expect("Failed to showcase deadlock visualization");
/// ```
pub fn showcase<P: AsRef<Path>>(log_path: P) -> Result<()> {
    // Process the log file to get an encoded string suitable for URLs
    let encoded_log =
        process_log_for_url(&log_path).context("Failed to process log file for URL")?;

    // Construct the URL with the encoded log as a parameter
    let showcase_url = format!("https://deloxide.vercel.app/?logs={encoded_log}");

    // Open the URL in the default web browser.
    webbrowser::open(&showcase_url).context("Failed to open browser")?;

    Ok(())
}

/// Showcase the current active log file
///
/// This is a convenience function that showcases the log file that was specified
/// in the Deloxide::with_log() initialization. It's useful when you don't want to
/// keep track of the log file path manually.
///
/// # Returns
/// A Result that is Ok if the showcase succeeded, or an error if it failed.
///
/// **IMPORTANT**: This function ensures all pending log entries are flushed to disk
/// before showcasing to guarantee the log file is complete.
///
/// # Errors
/// Returns an error if:
/// - No active log file exists
/// - Failed to flush pending log entries
/// - Failed to process the log file
/// - Failed to open the browser
///
/// # Example
///
/// ```no_run
/// use deloxide::{Deloxide, showcase_this};
///
/// // Initialize with logging enabled
/// Deloxide::new()
///     .with_log("deadlock_log.json")
///     .start()
///     .expect("Failed to initialize detector");
///
/// // Later, after a deadlock has been detected
/// showcase_this().expect("Failed to showcase current log");
/// ```
pub fn showcase_this() -> Result<()> {
    // First, flush all pending log entries to ensure completeness
    flush_global_detector_logs().context("Failed to flush pending log entries")?;

    // Get the current log file path
    let log_path = logger::get_current_log_file()
        .ok_or_else(|| anyhow::anyhow!("No active log file found"))?;

    // Process the log file to get an encoded string suitable for URLs
    let encoded_log =
        process_log_for_url(&log_path).context("Failed to process log file for URL")?;

    // Construct the URL with the encoded log as a parameter
    let showcase_url = format!("https://deloxide.vercel.app/?logs={encoded_log}");

    // Open the URL in the default web browser
    webbrowser::open(&showcase_url).context("Failed to open browser")?;

    Ok(())
}
