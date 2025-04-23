pub mod encoder;
pub use encoder::process_log_for_url;

use anyhow::{Context, Result};
use std::path::Path;
use crate::core::logger;

/// Showcase the log data by sending it to the showcase server
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
pub fn showcase<P: AsRef<Path>>(log_path: P) -> Result<()> {
    // Process the log file to get an encoded string suitable for URLs
    let encoded_log =
        process_log_for_url(&log_path).context("Failed to process log file for URL")?;

    // Construct the URL with the encoded log as a parameter
    let showcase_url = format!("https://deloxide.vercel.app/?logs={}", encoded_log);

    // Open the URL in the default web browser.
    webbrowser::open(&showcase_url).context("Failed to open browser")?;

    Ok(())
}

/// Showcase the current active log file
///
/// # Returns
/// A Result that is Ok if the showcase succeeded, or an error if it failed.
///
/// # Errors
/// Returns an error if:
/// - No active log file exists
/// - Failed to process the log file
/// - Failed to open the browser
pub fn showcase_this() -> Result<()> {
    // Get the current log file path
    let log_path = logger::get_current_log_file()
        .ok_or_else(|| anyhow::anyhow!("No active log file found"))?;

    // Process the log file to get an encoded string suitable for URLs
    let encoded_log = process_log_for_url(&log_path)
        .context("Failed to process log file for URL")?;

    // Construct the URL with the encoded log as a parameter
    let showcase_url = format!("https://deloxide.vercel.app/?logs={}", encoded_log);

    // Open the URL in the default web browser
    webbrowser::open(&showcase_url).context("Failed to open browser")?;

    Ok(())
}
