use crate::core::types::DeadlockInfo;
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
    let mut terminal_deadlock: Option<DeadlockCompact> = None;

    // Process each line
    for line in reader.lines() {
        let line = line.context("Failed to read line from log file")?;
        if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
            // Process each log entry
            let event = parse_log_entry(entry).context("Failed to parse log entry")?;
            compact_events.push(event);
        } else if let Ok(dl) = serde_json::from_str::<DeadlockRecord>(&line) {
            terminal_deadlock = Some(DeadlockCompact {
                thread_cycle: dl.deadlock.thread_cycle.iter().map(|&t| t as u64).collect(),
                thread_waiting_for_locks: dl
                    .deadlock
                    .thread_waiting_for_locks
                    .iter()
                    .map(|&(t, l)| (t as u64, l as u64))
                    .collect(),
                timestamp: dl.deadlock.timestamp.clone(),
            });
        }
    }

    // Encode as a fixed 2-tuple: [events, deadlock_or_null]
    let compact_output: (Events, Option<DeadlockCompact>) = (compact_events, terminal_deadlock);

    // 1. Convert to MessagePack
    let msgpack =
        rmp_serde::to_vec(&compact_output).context("Failed to convert data to MessagePack")?;

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
    /// Sequence number for deterministic ordering
    sequence: u64,
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
    /// Optional thread ID that was woken by condvar notify
    #[serde(default)]
    woken_thread: Option<u64>,
}

// Compact Event format: (sequence, thread_id, lock_id, event_code, timestamp, parent_id, woken_thread)
// parent_id and woken_thread are Option<u64> stored as u64, with 0 indicating None
type Event = (u64, u64, u64, u8, f64, u64, u64);

type Events = Vec<Event>;

#[derive(Serialize, Deserialize)]
pub struct DeadlockCompact {
    pub thread_cycle: Vec<u64>,
    pub thread_waiting_for_locks: Vec<(u64, u64)>,
    pub timestamp: String,
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

    // Convert parent_id and woken_thread to u64, using 0 to represent None
    let parent_id = entry.parent_id.unwrap_or(0);
    let woken_thread = entry.woken_thread.unwrap_or(0);

    let compact_event = (
        entry.sequence,
        entry.thread_id,
        entry.lock_id,
        event_code,
        entry.timestamp,
        parent_id,
        woken_thread,
    );

    Ok(compact_event)
}

#[derive(Deserialize)]
struct DeadlockRecord {
    deadlock: DeadlockInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_entry() {
        let log_entry = LogEntry {
            sequence: 42,
            thread_id: 1,
            lock_id: 2,
            event: "ThreadSpawn".to_string(),
            timestamp: 1754931441.237695,
            parent_id: Some(5),
            woken_thread: None,
        };

        let result = parse_log_entry(log_entry).expect("Failed to parse log entry");

        // Should be (sequence, thread_id, lock_id, event_code, timestamp, parent_id, woken_thread)
        assert_eq!(result.0, 42); // sequence
        assert_eq!(result.1, 1); // thread_id
        assert_eq!(result.2, 2); // lock_id
        assert_eq!(result.3, 0); // ThreadSpawn code
        assert_eq!(result.5, 5); // parent_id
        assert_eq!(result.6, 0); // woken_thread (None = 0)
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
                sequence: 0,
                thread_id: 1,
                lock_id: 2,
                event: event_name.to_string(),
                timestamp: 1754931441.237695,
                parent_id: None,
                woken_thread: None,
            };

            let result = parse_log_entry(log_entry)
                .unwrap_or_else(|_| panic!("Failed to parse {event_name}"));
            assert_eq!(
                result.3, expected_code,
                "Event {event_name} should have code {expected_code}",
            );
        }
    }
}
