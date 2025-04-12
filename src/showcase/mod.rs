pub mod encoder;
pub use encoder::process_log_for_url;

use std::path::Path;
use webbrowser;

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
pub fn showcase<P: AsRef<Path>>(log_path: P) -> Result<(), Box<dyn std::error::Error>> {
    // Process the log file to get an encoded string suitable for URLs
    let encoded_log = process_log_for_url(&log_path)?;

    // Construct the URL with the encoded log as a parameter
    let showcase_url = format!("https://deloxide.vercel.app/?data={}", encoded_log);

    // Open the URL in the default web browser.
    if webbrowser::open(&showcase_url).is_err() {
        eprintln!("Failed to open the browser automatically.");
        // Still print the URL for the user
        println!("Please open this URL manually: {}", showcase_url);
    }

    Ok(())
}

/// CLI function to handle showcase functionality.
pub fn cli_showcase<P: AsRef<Path>>(log_path: P) -> Result<(), Box<dyn std::error::Error>> {
    showcase(log_path)?;
    println!("Log data successfully sent to showcase!");
    Ok(())
}