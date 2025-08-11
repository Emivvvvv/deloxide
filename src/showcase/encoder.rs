use anyhow::{Context, Result};
use base64::alphabet::URL_SAFE;
use base64::engine::{Engine as _, general_purpose};
use flate2::Compression;
use flate2::write::GzEncoder;
use rmp_serde;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Converts a log file to a compact, compressed, encoded format suitable for URL parameters
///
/// This function processes a Deloxide log file and converts it into a format that can be
/// transmitted as a URL parameter for web-based visualization. It performs several steps:
///
/// 1. Parse the JSON log file into structured data
/// 2. Convert to a more compact representation
/// 3. Serialize to MessagePack binary format
/// 4. Compress using GZIP
/// 5. Encode using Base64URL for safe transmission in URLs
///
/// # Arguments
/// * `log_path` - Path to the original log file
///
/// # Returns
/// A Result that contains the encoded string or an error
///
/// # Errors
/// Returns an error if:
/// - Failed to open the log file
/// - Failed to read or parse the log file
/// - Failed to compress or encode the data
/// ```
pub(crate) fn process_log_for_url<P: AsRef<Path>>(log_path: P) -> Result<String> {
    // Parse the input file
    let file = File::open(log_path).context("Failed to open log file")?;
    let reader = BufReader::new(file);

    // Create compact data structure
    let mut compact_events = Vec::new();

    // Process each line
    for line in reader.lines() {
        let line = line.context("Failed to read line from log file")?;
        if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
            // Process each log entry
            let event = parse_log_entry(entry).context("Failed to parse log entry")?;
            compact_events.push(event);
        }
    }

    // Create the compact log data
    let compact_data = LogsData {
        events: compact_events,
    };

    // 1. Convert to MessagePack
    let msgpack =
        rmp_serde::to_vec(&compact_data).context("Failed to convert data to MessagePack")?;

    // 2. Apply Gzip compression
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&msgpack)
        .context("Failed to compress data")?;
    let compressed = encoder.finish().context("Failed to finish compression")?;

    // 3. Apply Base64URL encoding
    let base64_engine = general_purpose::GeneralPurpose::new(&URL_SAFE, general_purpose::PAD);
    let encoded = base64_engine.encode(compressed);

    Ok(encoded)
}

/// Log entry structure from the file (simplified - no graph data)
#[derive(Debug, Deserialize)]
struct LogEntry {
    /// ID of the thread involved in the event
    thread_id: u64,
    /// ID of the lock involved in the event
    lock_id: u64,
    /// Type of event
    event: String,
    /// Timestamp when the event occurred
    timestamp: f64,
    /// Optional parent/creator thread ID for Spawn events
    #[serde(default)]
    parent_id: Option<u64>,
}

// Compact Event format: (thread_id, lock_id, event_code, timestamp, parent_id)
// parent_id is Option<u64> stored as u64, with 0 indicating None
type Event = (u64, u64, u8, f64, u64);

type Events = Vec<Event>;

/// Compact output structure for serialization
#[derive(Serialize, Deserialize)]
pub struct LogsData {
    pub events: Events,
}

/// Parse a log entry into the compact format
///
/// This function converts a log entry from the JSON structure into a more compact
/// representation for efficient transmission. Each event type gets a unique code.
///
/// # Arguments
/// * `entry` - The log entry to parse
///
/// # Returns
/// A Result containing the parsed event, or an error
///
/// # Errors
/// Returns an error if the event type is invalid
fn parse_log_entry(entry: LogEntry) -> Result<Event> {
    // Convert event to compact format - each event type gets a unique code
    let event_code = match entry.event.as_str() {
        // Thread lifecycle
        "ThreadSpawn" => 0u8,
        "ThreadExit" => 1u8,

        // Mutex lifecycle
        "MutexSpawn" => 2u8,
        "MutexExit" => 3u8,

        // RwLock lifecycle
        "RwSpawn" => 4u8,
        "RwExit" => 5u8,

        // Condvar lifecycle
        "CondvarSpawn" => 6u8,
        "CondvarExit" => 7u8,

        // Mutex interactions
        "MutexAttempt" => 10u8,
        "MutexAcquired" => 11u8,
        "MutexReleased" => 12u8,

        // RwLock interactions
        "RwReadAttempt" => 20u8,
        "RwReadAcquired" => 21u8,
        "RwReadReleased" => 22u8,
        "RwWriteAttempt" => 23u8,
        "RwWriteAcquired" => 24u8,
        "RwWriteReleased" => 25u8,

        // Condvar interactions
        "CondvarWaitBegin" => 30u8,
        "CondvarWaitEnd" => 31u8,
        "CondvarNotifyOne" => 32u8,
        "CondvarNotifyAll" => 33u8,

        other => anyhow::bail!("Invalid event type: '{}'", other),
    };

    // Convert parent_id to u64, using 0 to represent None
    let parent_id = entry.parent_id.unwrap_or(0);

    let compact_event = (
        entry.thread_id,
        entry.lock_id,
        event_code,
        entry.timestamp,
        parent_id,
    );

    Ok(compact_event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_entry() {
        let log_entry = LogEntry {
            thread_id: 1,
            lock_id: 2,
            event: "ThreadSpawn".to_string(),
            timestamp: 1754931441.237695,
            parent_id: Some(5),
        };

        let result = parse_log_entry(log_entry).expect("Failed to parse log entry");

        // Should be (thread_id, lock_id, event_code, timestamp, parent_id)
        assert_eq!(result.0, 1); // thread_id
        assert_eq!(result.1, 2); // lock_id
        assert_eq!(result.2, 0); // ThreadSpawn code
        assert_eq!(result.4, 5); // parent_id
    }

    #[test]
    fn test_unique_event_codes() {
        let events = vec![
            ("ThreadSpawn", 0u8),
            ("ThreadExit", 1u8),
            ("MutexSpawn", 2u8),
            ("MutexExit", 3u8),
            ("RwSpawn", 4u8),
            ("RwExit", 5u8),
            ("CondvarSpawn", 6u8),
            ("CondvarExit", 7u8),
            ("MutexAttempt", 10u8),
            ("MutexAcquired", 11u8),
            ("MutexReleased", 12u8),
            ("RwReadAttempt", 20u8),
            ("RwReadAcquired", 21u8),
            ("RwReadReleased", 22u8),
            ("RwWriteAttempt", 23u8),
            ("RwWriteAcquired", 24u8),
            ("RwWriteReleased", 25u8),
            ("CondvarWaitBegin", 30u8),
            ("CondvarWaitEnd", 31u8),
            ("CondvarNotifyOne", 32u8),
            ("CondvarNotifyAll", 33u8),
        ];

        for (event_name, expected_code) in events {
            let log_entry = LogEntry {
                thread_id: 1,
                lock_id: 2,
                event: event_name.to_string(),
                timestamp: 1754931441.237695,
                parent_id: None,
            };

            let result =
                parse_log_entry(log_entry).expect(&format!("Failed to parse {}", event_name));
            assert_eq!(
                result.2, expected_code,
                "Event {event_name} should have code {expected_code}",
            );
        }
    }
}
