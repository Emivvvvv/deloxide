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
///
/// # Example
///
/// ```no_run
/// use deloxide::process_log_for_url;
///
/// let encoded = process_log_for_url("deadlock_log.json")
///     .expect("Failed to process log file");
///
/// // The encoded string can be appended to a URL
/// let showcase_url = format!("https://example.com/visualize?data={}", encoded);
/// ```
pub fn process_log_for_url<P: AsRef<Path>>(log_path: P) -> Result<String> {
    // Parse the input file
    let file = File::open(log_path).context("Failed to open log file")?;
    let reader = BufReader::new(file);

    // Create compact data structure
    let mut compact_events = Vec::new();
    let mut compact_graphs = Vec::new();

    // Process each line
    for line in reader.lines() {
        let line = line.context("Failed to read line from log file")?;
        if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
            // Process each log entry
            let (event, graph) = parse_log_entry(entry).context("Failed to parse log entry")?;
            compact_events.push(event);
            compact_graphs.push(graph);
        }
    }

    // Create the compact log data
    let compact_data = LogsData {
        events: compact_events,
        graphs: compact_graphs,
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

/// Original log entry structure from the file
#[derive(Debug, Deserialize)]
struct LogEntry {
    event: EventData,
    graph: GraphData,
}

/// Event data from the log file
#[derive(Debug, Deserialize)]
struct EventData {
    /// ID of the thread involved in the event
    thread_id: u64,
    /// ID of the lock involved in the event
    lock_id: u64,
    /// Type of event (Spawn, Exit, Attempt, Acquired, Released)
    event: String,
    /// Timestamp when the event occurred
    timestamp: f64,
    /// Optional parent/creator thread ID for Spawn events
    #[serde(default)]
    parent_id: Option<u64>,
}

/// Graph data from the log file
#[derive(Debug, Deserialize)]
struct GraphData {
    /// List of active thread IDs
    threads: Vec<u64>,
    /// List of active lock IDs
    locks: Vec<u64>,
    /// Links between threads and locks
    links: Vec<LinkData>,
}

/// Link data representing relationships between threads and locks
#[derive(Debug, Deserialize)]
struct LinkData {
    /// Source thread ID
    source: u64,
    /// Target lock ID
    target: u64,
    /// Type of relationship (Attempt, Acquired, Created)
    #[serde(rename = "type")]
    link_type: String,
}

// Compact Event format: (thread_id, lock_id, event_code, timestamp, parent_id)
// parent_id is Option<u64> stored as u64, with 0 indicating None
type Event = (u64, u64, u8, f64, u64);

// Compact Graph format: (threads, locks, links)
// links are (source, target, link_type_code)
type Graph = (Vec<u64>, Vec<u64>, Vec<(u64, u64, u8)>);

type Events = Vec<Event>;
type Graphs = Vec<Graph>;

/// Compact output structure for serialization
#[derive(Serialize, Deserialize)]
pub struct LogsData {
    pub events: Events,
    pub graphs: Graphs,
}

/// Parse a log entry into the compact format
///
/// This function converts a log entry from the JSON structure into a more compact
/// representation for efficient transmission. Event types and link types are mapped
/// to numeric codes to save space.
///
/// # Arguments
/// * `entry` - The log entry to parse
///
/// # Returns
/// A Result containing the parsed event and graph, or an error
///
/// # Errors
/// Returns an error if the event type or link type is invalid
fn parse_log_entry(entry: LogEntry) -> Result<(Event, Graph)> {
    // Convert event to compact format
    let event_code = match entry.event.event.as_str() {
        "Spawn" => 3u8, // Code for Spawn events
        "Exit" => 4u8,  // Code for Exit events
        "Attempt" => 0u8,
        "Acquired" => 1u8,
        "Released" => 2u8,
        other => anyhow::bail!("Invalid event type: '{}'", other),
    };

    // Convert parent_id to u64, using 0 to represent None
    let parent_id = entry.event.parent_id.unwrap_or(0);

    let compact_event = (
        entry.event.thread_id,
        entry.event.lock_id,
        event_code,
        entry.event.timestamp,
        parent_id,
    );

    // Convert graph to compact format
    let mut compact_links = Vec::new();
    for link in entry.graph.links {
        let link_type_code = match link.link_type.as_str() {
            "Attempt" | "attempt" => 0u8,
            "Acquired" | "acquired" => 1u8,
            "Created" | "created" => 2u8,
            "Read" | "read" => 3u8,
            "Write" | "write" => 4u8,
            "Wait" | "wait" => 5u8,
            _ => anyhow::bail!("Invalid link type: {}", link.link_type),
        };

        compact_links.push((link.source, link.target, link_type_code));
    }

    let compact_graph = (entry.graph.threads, entry.graph.locks, compact_links);

    Ok((compact_event, compact_graph))
}
