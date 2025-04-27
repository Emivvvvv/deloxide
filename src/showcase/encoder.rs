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
    #[serde(default)]
    parent_id: Option<u64>,
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

// Event format: (thread_id, lock_id, event_code, timestamp, parent_id)
// parent_id is Option<u64> stored as u64, with 0 indicating None
type Event = (u64, u64, u8, f64, u64);
// Graph format: (threads, locks, links)
type Graph = (Vec<u64>, Vec<u64>, Vec<(u64, u64, u8)>);

type Events = Vec<Event>;
type Graphs = Vec<Graph>;

/// Compact output structure
#[derive(Serialize, Deserialize)]
pub struct LogsData {
    pub events: Events,
    pub graphs: Graphs,
}

/// Parse a log entry into the compact format
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
            "Created" | "created" => 2u8, // New code for Created relationship
            _ => anyhow::bail!("Invalid link type: {}", link.link_type),
        };

        compact_links.push((link.source, link.target, link_type_code));
    }

    let compact_graph = (entry.graph.threads, entry.graph.locks, compact_links);

    Ok((compact_event, compact_graph))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper function to create a temporary log file with test data
    fn create_test_log_file(entries: &[&str]) -> Result<NamedTempFile> {
        let mut file = NamedTempFile::new()?;
        for entry in entries {
            writeln!(file, "{}", entry)?;
        }
        file.flush()?;
        Ok(file)
    }

    // Helper function to decode the output back to LogsData
    fn decode_url_data(encoded: &str) -> Result<LogsData> {
        // 1. Base64URL decode
        let base64_engine = general_purpose::GeneralPurpose::new(&URL_SAFE, general_purpose::PAD);
        let compressed = base64_engine.decode(encoded)?;

        // 2. Gunzip decompress
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut msgpack = Vec::new();
        decoder.read_to_end(&mut msgpack)?;

        // 3. MessagePack deserialize
        let logs_data: LogsData = rmp_serde::from_slice(&msgpack)?;
        Ok(logs_data)
    }

    #[test]
    fn test_parse_lock_spawn_with_creator() -> Result<()> {
        let entry = LogEntry {
            event: EventData {
                thread_id: 0, // Thread ID 0 indicates a lock-only event
                lock_id: 123, // The lock ID that was created
                event: "Spawn".to_string(),
                timestamp: 1234567890.123,
                parent_id: Some(456), // Creator thread ID
            },
            graph: GraphData {
                threads: vec![456],
                locks: vec![123],
                links: vec![LinkData {
                    source: 456,
                    target: 123,
                    link_type: "Created".to_string(),
                }],
            },
        };

        let (event, graph) = parse_log_entry(entry)?;

        // Verify event
        assert_eq!(event.0, 0); // thread_id (0 for lock-only event)
        assert_eq!(event.1, 123); // lock_id
        assert_eq!(event.2, 3); // event_code for Spawn
        assert_eq!(event.3, 1234567890.123); // timestamp
        assert_eq!(event.4, 456); // parent_id/creator_id

        // Verify graph
        assert_eq!(graph.0, vec![456]); // Creator thread
        assert_eq!(graph.1, vec![123]); // Lock exists in graph

        // Verify Created link
        assert_eq!(graph.2.len(), 1);
        assert_eq!(graph.2[0].0, 456); // Creator thread
        assert_eq!(graph.2[0].1, 123); // Lock ID
        assert_eq!(graph.2[0].2, 2); // Created link type

        Ok(())
    }

    #[test]
    fn test_parse_thread_spawn_with_parent() -> Result<()> {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123, // Thread ID that was created
                lock_id: 0,     // Lock ID 0 indicates a thread-only event
                event: "Spawn".to_string(),
                timestamp: 1234567890.123,
                parent_id: Some(456), // Parent thread ID
            },
            graph: GraphData {
                threads: vec![123, 456],
                locks: vec![],
                links: vec![],
            },
        };

        let (event, _) = parse_log_entry(entry)?;

        // Verify event
        assert_eq!(event.0, 123); // thread_id
        assert_eq!(event.1, 0); // lock_id (0 for thread-only event)
        assert_eq!(event.2, 3); // event_code for Spawn
        assert_eq!(event.3, 1234567890.123); // timestamp
        assert_eq!(event.4, 456); // parent_id

        Ok(())
    }

    #[test]
    fn test_resource_ownership_lifecycle() -> Result<()> {
        // Create a test log that includes resource ownership lifecycle
        let entries = [
            // Parent thread is created
            r#"{"event":{"thread_id":1,"lock_id":0,"event":"Spawn","timestamp":1234567890.000,"parent_id":null},"graph":{"threads":[1],"locks":[],"links":[]}}"#,
            // Parent thread creates a lock
            r#"{"event":{"thread_id":0,"lock_id":10,"event":"Spawn","timestamp":1234567890.050,"parent_id":1},"graph":{"threads":[1],"locks":[10],"links":[{"source":1,"target":10,"type":"Created"}]}}"#,
            // Parent thread spawns a child thread
            r#"{"event":{"thread_id":2,"lock_id":0,"event":"Spawn","timestamp":1234567890.100,"parent_id":1},"graph":{"threads":[1,2],"locks":[10],"links":[{"source":1,"target":10,"type":"Created"}]}}"#,
            // Child thread attempts to acquire lock
            r#"{"event":{"thread_id":2,"lock_id":10,"event":"Attempt","timestamp":1234567890.150,"parent_id":null},"graph":{"threads":[1,2],"locks":[10],"links":[{"source":1,"target":10,"type":"Created"},{"source":2,"target":10,"type":"Attempt"}]}}"#,
            // Child thread acquires lock
            r#"{"event":{"thread_id":2,"lock_id":10,"event":"Acquired","timestamp":1234567890.200,"parent_id":null},"graph":{"threads":[1,2],"locks":[10],"links":[{"source":1,"target":10,"type":"Created"},{"source":2,"target":10,"type":"Acquired"}]}}"#,
            // Parent thread exits - should not affect lock since child still references it
            r#"{"event":{"thread_id":1,"lock_id":0,"event":"Exit","timestamp":1234567890.250,"parent_id":null},"graph":{"threads":[2],"locks":[10],"links":[{"source":2,"target":10,"type":"Acquired"}]}}"#,
            // Child thread releases lock
            r#"{"event":{"thread_id":2,"lock_id":10,"event":"Released","timestamp":1234567890.300,"parent_id":null},"graph":{"threads":[2],"locks":[10],"links":[]}}"#,
            // Child thread exits - now lock should be destroyed
            r#"{"event":{"thread_id":2,"lock_id":0,"event":"Exit","timestamp":1234567890.350,"parent_id":null},"graph":{"threads":[],"locks":[],"links":[]}}"#,
            // Lock is destroyed
            r#"{"event":{"thread_id":0,"lock_id":10,"event":"Exit","timestamp":1234567890.400,"parent_id":null},"graph":{"threads":[],"locks":[],"links":[]}}"#,
        ];

        let file = create_test_log_file(&entries)?;
        let encoded = process_log_for_url(file.path())?;

        // Decode and verify
        let logs_data = decode_url_data(&encoded)?;
        assert_eq!(logs_data.events.len(), 9);
        assert_eq!(logs_data.graphs.len(), 9);

        // Verify parent-child relationship
        assert_eq!(logs_data.events[2].4, 1); // Child thread has parent ID 1

        // Verify creator-resource relationship
        assert_eq!(logs_data.events[1].4, 1); // Lock has creator ID 1

        // Verify the "Created" link type was correctly encoded
        assert!(logs_data.graphs[1].2.iter().any(
            |&(src, tgt, typ)| src == 1 && tgt == 10 && typ == 2 // typ 2 = Created
        ));

        // Verify lock is removed after both parent and child threads exit
        assert!(logs_data.graphs[8].1.is_empty()); // No locks in final graph

        Ok(())
    }

    #[test]
    fn test_parse_log_entry_attempt() -> Result<()> {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123,
                lock_id: 456,
                event: "Attempt".to_string(),
                timestamp: 1234567890.123,
                parent_id: None,
            },
            graph: GraphData {
                threads: vec![123, 789],
                locks: vec![456, 654],
                links: vec![LinkData {
                    source: 123,
                    target: 456,
                    link_type: "Attempt".to_string(),
                }],
            },
        };

        let (event, graph) = parse_log_entry(entry)?;

        // Verify event
        assert_eq!(event.0, 123); // thread_id
        assert_eq!(event.1, 456); // lock_id
        assert_eq!(event.2, 0); // event_code for Attempt
        assert_eq!(event.3, 1234567890.123); // timestamp
        assert_eq!(event.4, 0); // No parent ID

        // Verify graph
        assert_eq!(graph.0, vec![123, 789]); // threads
        assert_eq!(graph.1, vec![456, 654]); // locks
        assert_eq!(graph.2, vec![(123, 456, 0)]); // links (with type code 0 for Attempt)

        Ok(())
    }

    #[test]
    fn test_parse_log_entry_acquired() -> Result<()> {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123,
                lock_id: 456,
                event: "Acquired".to_string(),
                timestamp: 1234567890.123,
                parent_id: None,
            },
            graph: GraphData {
                threads: vec![123],
                locks: vec![456],
                links: vec![LinkData {
                    source: 123,
                    target: 456,
                    link_type: "Acquired".to_string(),
                }],
            },
        };

        let (event, graph) = parse_log_entry(entry)?;

        // Verify event code for Acquired
        assert_eq!(event.2, 1);
        // Verify link type code for Acquired
        assert_eq!(graph.2[0].2, 1);

        Ok(())
    }

    #[test]
    fn test_parse_log_entry_released() -> Result<()> {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123,
                lock_id: 456,
                event: "Released".to_string(),
                timestamp: 1234567890.123,
                parent_id: None,
            },
            graph: GraphData {
                threads: vec![123],
                locks: vec![456],
                links: vec![],
            },
        };

        let (event, _) = parse_log_entry(entry)?;

        // Verify event code for Released
        assert_eq!(event.2, 2);

        Ok(())
    }

    #[test]
    fn test_parse_log_entry_invalid_event() {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123,
                lock_id: 456,
                event: "Invalid".to_string(),
                timestamp: 1234567890.123,
                parent_id: None,
            },
            graph: GraphData {
                threads: vec![123],
                locks: vec![456],
                links: vec![],
            },
        };

        let result = parse_log_entry(entry);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid event type")
        );
    }

    #[test]
    fn test_parse_log_entry_invalid_link_type() {
        let entry = LogEntry {
            event: EventData {
                thread_id: 123,
                lock_id: 456,
                event: "Acquired".to_string(),
                timestamp: 1234567890.123,
                parent_id: None,
            },
            graph: GraphData {
                threads: vec![123],
                locks: vec![456],
                links: vec![LinkData {
                    source: 123,
                    target: 456,
                    link_type: "Invalid".to_string(),
                }],
            },
        };

        let result = parse_log_entry(entry);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid link type")
        );
    }

    #[test]
    fn test_process_log_for_url_empty_file() -> Result<()> {
        let file = create_test_log_file(&[])?;
        let encoded = process_log_for_url(file.path())?;

        // Even an empty file should produce valid base64
        assert!(!encoded.is_empty());

        // Decode and verify
        let logs_data = decode_url_data(&encoded)?;
        assert!(logs_data.events.is_empty());
        assert!(logs_data.graphs.is_empty());

        Ok(())
    }

    #[test]
    fn test_process_log_for_url_single_entry() -> Result<()> {
        let json_entry = r#"{"event":{"thread_id":123,"lock_id":456,"event":"Acquired","timestamp":1234567890.123,"parent_id":null},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Acquired"}]}}"#;
        let file = create_test_log_file(&[json_entry])?;

        let encoded = process_log_for_url(file.path())?;
        assert!(!encoded.is_empty());

        // Decode and verify
        let logs_data = decode_url_data(&encoded)?;
        assert_eq!(logs_data.events.len(), 1);
        assert_eq!(logs_data.graphs.len(), 1);

        // Check specific values
        let event = logs_data.events[0];
        assert_eq!(event.0, 123); // thread_id
        assert_eq!(event.1, 456); // lock_id
        assert_eq!(event.2, 1); // event_code for Acquired
        assert_eq!(event.4, 0); // No parent ID

        Ok(())
    }

    #[test]
    fn test_process_log_for_url_multiple_entries() -> Result<()> {
        let entries = [
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Attempt","timestamp":1234567890.000,"parent_id":null},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Attempt"}]}}"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Acquired","timestamp":1234567890.100,"parent_id":null},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Acquired"}]}}"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Released","timestamp":1234567890.200,"parent_id":null},"graph":{"threads":[123],"locks":[456],"links":[]}}"#,
        ];

        let file = create_test_log_file(&entries)?;
        let encoded = process_log_for_url(file.path())?;

        // Decode and verify
        let logs_data = decode_url_data(&encoded)?;
        assert_eq!(logs_data.events.len(), 3);
        assert_eq!(logs_data.graphs.len(), 3);

        // Check the sequence of events
        assert_eq!(logs_data.events[0].2, 0); // Attempt
        assert_eq!(logs_data.events[1].2, 1); // Acquired
        assert_eq!(logs_data.events[2].2, 2); // Released

        Ok(())
    }

    #[test]
    fn test_process_log_for_url_invalid_json() -> Result<()> {
        let entries = [
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Attempt","timestamp":1234567890.000},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Attempt"}]}}"#,
            r#"This is not valid JSON"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Released","timestamp":1234567890.200},"graph":{"threads":[123],"locks":[456],"links":[]}}"#,
        ];

        let file = create_test_log_file(&entries)?;
        let encoded = process_log_for_url(file.path())?;

        // Decode and verify - should only have 2 valid entries
        let logs_data = decode_url_data(&encoded)?;
        assert_eq!(logs_data.events.len(), 2);
        assert_eq!(logs_data.graphs.len(), 2);

        Ok(())
    }

    #[test]
    #[should_panic(expected = "Invalid event type")]
    fn test_process_log_for_url_invalid_event_type() {
        let entries = [
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Attempt","timestamp":1234567890.000},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Attempt"}]}}"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"InvalidEvent","timestamp":1234567890.100},"graph":{"threads":[123],"locks":[456],"links":[{"source":123,"target":456,"type":"Attempt"}]}}"#,
        ];

        let file = create_test_log_file(&entries).unwrap();

        // This will panic with "Invalid event type" message
        let _ = process_log_for_url(file.path()).unwrap();
    }

    #[test]
    fn test_file_not_found() {
        let result = process_log_for_url("non_existent_file.log");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to open log file")
        );
    }

    #[test]
    fn test_round_trip_encoding_decoding() -> Result<()> {
        // Create a complex log with multiple entries
        let entries = [
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Attempt","timestamp":1234567890.000,"parent_id":null},"graph":{"threads":[123,789],"locks":[456,654],"links":[{"source":123,"target":456,"type":"Attempt"}]}}"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Acquired","timestamp":1234567890.100,"parent_id":null},"graph":{"threads":[123,789],"locks":[456,654],"links":[{"source":123,"target":456,"type":"Acquired"}]}}"#,
            r#"{"event":{"thread_id":789,"lock_id":654,"event":"Attempt","timestamp":1234567890.150,"parent_id":null},"graph":{"threads":[123,789],"locks":[456,654],"links":[{"source":123,"target":456,"type":"Acquired"},{"source":789,"target":654,"type":"Attempt"}]}}"#,
            r#"{"event":{"thread_id":789,"lock_id":654,"event":"Acquired","timestamp":1234567890.200,"parent_id":null},"graph":{"threads":[123,789],"locks":[456,654],"links":[{"source":123,"target":456,"type":"Acquired"},{"source":789,"target":654,"type":"Acquired"}]}}"#,
            r#"{"event":{"thread_id":123,"lock_id":456,"event":"Released","timestamp":1234567890.300,"parent_id":null},"graph":{"threads":[123,789],"locks":[456,654],"links":[{"source":789,"target":654,"type":"Acquired"}]}}"#,
        ];

        let file = create_test_log_file(&entries)?;
        let encoded = process_log_for_url(file.path())?;

        // Decode and verify full round-trip
        let logs_data = decode_url_data(&encoded)?;

        // Verify correct number of entries
        assert_eq!(logs_data.events.len(), 5);
        assert_eq!(logs_data.graphs.len(), 5);

        // Verify specific events in sequence
        assert_eq!(logs_data.events[0].2, 0); // Attempt
        assert_eq!(logs_data.events[1].2, 1); // Acquired
        assert_eq!(logs_data.events[2].2, 0); // Attempt
        assert_eq!(logs_data.events[3].2, 1); // Acquired
        assert_eq!(logs_data.events[4].2, 2); // Released

        // Verify thread IDs
        assert_eq!(logs_data.events[0].0, 123);
        assert_eq!(logs_data.events[2].0, 789);

        // Verify timestamps are preserved
        assert_eq!(logs_data.events[0].3, 1234567890.000);
        assert_eq!(logs_data.events[4].3, 1234567890.300);

        Ok(())
    }
}
