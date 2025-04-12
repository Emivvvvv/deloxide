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
/// # Arguments
/// * `log_path` - Path to the original log file
///
/// # Returns
/// A Result that contains the encoded string or an error
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

#[derive(Debug, Deserialize)]
struct EventData {
    thread_id: u64,
    lock_id: u64,
    event: String,
    timestamp: f64,
}

#[derive(Debug, Deserialize)]
struct GraphData {
    threads: Vec<u64>,
    locks: Vec<u64>,
    links: Vec<LinkData>,
}

#[derive(Debug, Deserialize)]
struct LinkData {
    source: u64,
    target: u64,
    #[serde(rename = "type")]
    link_type: String,
}

type Event = (u64, u64, u8, f64);
type Graph = (Vec<u64>, Vec<u64>, Vec<(u64, u64, u8)>);

type Events = Vec<Event>;
type Graphs = Vec<Graph>;

/// Compact output structure
#[derive(Serialize)]
pub struct LogsData {
    pub events: Events,
    pub graphs: Graphs,
}

/// Parse a log entry into the compact format
fn parse_log_entry(entry: LogEntry) -> Result<(Event, Graph)> {
    // Convert event to compact format
    let event_code = match entry.event.event.as_str() {
        "Attempt" => 0u8,
        "Acquired" => 1u8,
        "Released" => 2u8,
        _ => anyhow::bail!("Invalid event type: {}", entry.event.event),
    };

    let compact_event = (
        entry.event.thread_id,
        entry.event.lock_id,
        event_code,
        entry.event.timestamp,
    );

    // Convert graph to compact format
    let mut compact_links = Vec::new();
    for link in entry.graph.links {
        let link_type_code = match link.link_type.as_str() {
            "Attempt" | "attempt" => 0u8,
            "Acquired" | "acquired" => 1u8,
            _ => anyhow::bail!("Invalid link type: {}", link.link_type),
        };

        compact_links.push((link.source, link.target, link_type_code));
    }

    let compact_graph = (entry.graph.threads, entry.graph.locks, compact_links);

    Ok((compact_event, compact_graph))
}
