use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use reqwest::blocking::Client;
use serde::Deserialize;

// Define a struct for deserializing the server response.
#[derive(Deserialize)]
struct ShowcaseResponse {
    id: String,
}

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
/// - Failed to send the log data to the server
pub fn showcase<P: AsRef<Path>>(log_path: P) -> Result<(), Box<dyn std::error::Error>> {
    // Convert provided log_path into a PathBuf.
    let path = log_path.as_ref().to_path_buf();

    // Read the log file
    let log_content = read_log_file(path)?;

    // Send the log content to the showcase server and open the resulting URL
    send_log_to_showcase(log_content)?;

    Ok(())
}

/// Function to read a log file and return its contents.
fn read_log_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

/// Sends log data to the showcase server and opens the browser to the resulting page.
fn send_log_to_showcase(log_content: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client
        .post("https://deloxide.vercel.app/showcase")
        .body(log_content)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("Server responded with error: {}", response.status()).into());
    }

    // Parse the response to get the unique id
    let showcase_response: ShowcaseResponse = response.json()?;

    // Construct the URL using the unique identifier
    let showcase_url = format!("https://deloxide.vercel.app/showcase/{}", showcase_response.id);

    // Open the URL in the default web browser.
    if webbrowser::open(&showcase_url).is_err() {
        eprintln!("Failed to open the browser automatically.");
    }

    Ok(())
}

/// CLI function to handle showcase functionality.
pub fn cli_showcase<P: AsRef<Path>>(log_path: P) -> Result<(), Box<dyn std::error::Error>> {
    let log_content = read_log_file(log_path)?;
    send_log_to_showcase(log_content)?;
    println!("Log data successfully sent to showcase!");
    Ok(())
}
