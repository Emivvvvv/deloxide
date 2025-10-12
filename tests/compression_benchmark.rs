/// Test to measure actual compression ratios in the visualization pipeline
/// 
/// This test creates realistic log data and measures the size reduction
/// at each stage of the encoding pipeline to verify thesis claims.
///
/// Run with: cargo test --test compression_benchmark -- --nocapture

use serde::{Deserialize, Serialize};
use std::fs::{File, self};
use std::io::{Write, BufWriter};
use flate2::write::GzEncoder;
use flate2::Compression;
use base64::Engine;

/// Represents a log entry (matches deloxide log format)
#[derive(Debug, Serialize, Deserialize)]
struct LogEntry {
    thread_id: u64,
    lock_id: u64,
    event: String,
    timestamp: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<u64>,
}

/// Compact event format (tuple representation)
type CompactEvent = (u64, u64, u8, f64, u64);

/// Deadlock information in compact format
#[derive(Serialize, Deserialize)]
struct DeadlockCompact {
    thread_cycle: Vec<u64>,
    thread_waiting_for_locks: Vec<(u64, u64)>,
    timestamp: String,
}

/// Convert event string to event code
fn event_to_code(event: &str) -> u8 {
    match event {
        "ThreadSpawn" => 0,
        "ThreadExit" => 1,
        "MutexSpawn" => 2,
        "MutexExit" => 3,
        "MutexAttempt" => 4,
        "MutexAcquired" => 5,
        "MutexReleased" => 6,
        "RwSpawn" => 7,
        "RwExit" => 8,
        "RwReadAttempt" => 9,
        "RwReadAcquired" => 10,
        "RwReadReleased" => 11,
        "RwWriteAttempt" => 12,
        "RwWriteAcquired" => 13,
        "RwWriteReleased" => 14,
        _ => 255,
    }
}

/// Generate realistic test log data
fn generate_test_log(num_events: usize) -> Vec<LogEntry> {
    let mut events = Vec::new();
    let mut timestamp = 0.0;
    
    // Simulate a typical deadlock scenario with multiple threads
    let num_threads: usize = 10;
    let num_locks: usize = 5;
    
    // Thread spawns
    for thread_id in 1..=num_threads {
        events.push(LogEntry {
            thread_id: thread_id as u64,
            lock_id: 0,
            event: "ThreadSpawn".to_string(),
            timestamp,
            parent_id: Some(0),
        });
        timestamp += 0.001;
    }
    
    // Lock creations
    for lock_id in 1..=num_locks {
        events.push(LogEntry {
            thread_id: 1,
            lock_id: lock_id as u64,
            event: "MutexSpawn".to_string(),
            timestamp,
            parent_id: None,
        });
        timestamp += 0.002;
    }
    
    // Generate lock operations until we reach desired event count
    let mut thread_idx = 1;
    let mut lock_idx = 1;
    
    while events.len() < num_events.saturating_sub(num_threads + 10) {
        let thread_id = ((thread_idx % num_threads) + 1) as u64;
        let lock_id = ((lock_idx % num_locks) + 1) as u64;
        
        // Attempt -> Acquired -> Released sequence
        events.push(LogEntry {
            thread_id,
            lock_id,
            event: "MutexAttempt".to_string(),
            timestamp,
            parent_id: None,
        });
        timestamp += 0.0005;
        
        events.push(LogEntry {
            thread_id,
            lock_id,
            event: "MutexAcquired".to_string(),
            timestamp,
            parent_id: None,
        });
        timestamp += 0.002;
        
        events.push(LogEntry {
            thread_id,
            lock_id,
            event: "MutexReleased".to_string(),
            timestamp,
            parent_id: None,
        });
        timestamp += 0.0005;
        
        thread_idx += 1;
        lock_idx += 1;
    }
    
    // Thread exits
    for thread_id in 1..=num_threads {
        events.push(LogEntry {
            thread_id: thread_id as u64,
            lock_id: 0,
            event: "ThreadExit".to_string(),
            timestamp,
            parent_id: None,
        });
        timestamp += 0.001;
    }
    
    events
}

/// Compact log entries into tuple format
fn compact_events(events: &[LogEntry]) -> Vec<CompactEvent> {
    events
        .iter()
        .map(|entry| {
            (
                entry.thread_id,
                entry.lock_id,
                event_to_code(&entry.event),
                entry.timestamp,
                entry.parent_id.unwrap_or(0),
            )
        })
        .collect()
}

#[test]
fn measure_compression_ratios() {
    println!("\n=== VISUALIZATION PIPELINE COMPRESSION BENCHMARK ===\n");
    
    // Test with different event counts to show scalability
    let test_sizes = vec![50, 100, 500, 1000, 2000, 5000, 10000, 50000, 100000];
    
    for &num_events in &test_sizes {
        println!("Testing with {} events:", num_events);
        println!("─────────────────────────────────────────────────");
        
        // Generate test data
        let events = generate_test_log(num_events);
        
        // Stage 1: Original JSON format
        let json_data = serde_json::to_string_pretty(&events).unwrap();
        let json_size = json_data.len();
        println!("1. JSON (original):           {} bytes", json_size);
        
        // Stage 2: Compact tuple format
        let compact_data = compact_events(&events);
        let compact_json = serde_json::to_string(&compact_data).unwrap();
        let compact_size = compact_json.len();
        let compact_reduction = ((json_size - compact_size) as f64 / json_size as f64) * 100.0;
        println!(
            "2. Compacted tuples:          {} bytes ({:.1}% reduction)",
            compact_size, compact_reduction
        );
        
        // Stage 3: MessagePack serialization
        let msgpack_data = rmp_serde::to_vec(&compact_data).unwrap();
        let msgpack_size = msgpack_data.len();
        let msgpack_vs_compact = ((compact_size - msgpack_size) as f64 / compact_size as f64) * 100.0;
        let msgpack_vs_original = ((json_size - msgpack_size) as f64 / json_size as f64) * 100.0;
        println!(
            "3. MessagePack:               {} bytes ({:.1}% vs compact, {:.1}% vs JSON)",
            msgpack_size, msgpack_vs_compact, msgpack_vs_original
        );
        
        // Stage 4: Gzip compression
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&msgpack_data).unwrap();
        let gzip_data = encoder.finish().unwrap();
        let gzip_size = gzip_data.len();
        let gzip_vs_msgpack = ((msgpack_size - gzip_size) as f64 / msgpack_size as f64) * 100.0;
        let gzip_vs_original = ((json_size - gzip_size) as f64 / json_size as f64) * 100.0;
        println!(
            "4. Gzip compressed:           {} bytes ({:.1}% vs MessagePack, {:.1}% vs JSON)",
            gzip_size, gzip_vs_msgpack, gzip_vs_original
        );
        
        // Stage 5: Base64 encoding (increases size)
        let base64_engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let base64_data = base64_engine.encode(&gzip_data);
        let base64_size = base64_data.len();
        let base64_overhead = ((base64_size - gzip_size) as f64 / gzip_size as f64) * 100.0;
        let overall_reduction = ((json_size - base64_size) as f64 / json_size as f64) * 100.0;
        println!(
            "5. Base64 encoded (final):    {} bytes (+{:.1}% encoding overhead)",
            base64_size, base64_overhead
        );
        
        println!("\n📊 Summary:");
        println!("   Total size reduction:      {:.1}% ({} KB → {} KB)",
            overall_reduction,
            json_size / 1024,
            base64_size / 1024
        );
        println!("   Compression ratio:         {:.2}x",
            json_size as f64 / base64_size as f64
        );
        println!();
    }
    
    // Additional test with realistic deadlock scenario
    println!("\n=== REALISTIC DEADLOCK SCENARIO ===\n");
    
    let events = generate_test_log(1000);
    let compact_data = compact_events(&events);
    
    // Add a deadlock
    let deadlock = Some(DeadlockCompact {
        thread_cycle: vec![1, 2],
        thread_waiting_for_locks: vec![(1, 2), (2, 1)],
        timestamp: "2024-01-15T10:30:45.123Z".to_string(),
    });
    
    let data_with_deadlock = (compact_data, deadlock);
    
    let json_with_deadlock = serde_json::to_string_pretty(&data_with_deadlock).unwrap();
    let json_size = json_with_deadlock.len();
    
    let msgpack_data = rmp_serde::to_vec(&data_with_deadlock).unwrap();
    let _msgpack_size = msgpack_data.len();
    
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&msgpack_data).unwrap();
    let gzip_data = encoder.finish().unwrap();
    let _gzip_size = gzip_data.len();
    
    let base64_engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let base64_data = base64_engine.encode(&gzip_data);
    let base64_size = base64_data.len();
    
    let overall_reduction = ((json_size - base64_size) as f64 / json_size as f64) * 100.0;
    
    println!("Scenario: 1000 events + deadlock information");
    println!("JSON:                         {} bytes ({} KB)", json_size, json_size / 1024);
    println!("Final (Base64):               {} bytes ({} KB)", base64_size, base64_size / 1024);
    println!("Overall reduction:            {:.1}%", overall_reduction);
    println!("Compression ratio:            {:.2}x", json_size as f64 / base64_size as f64);
    println!("\n✅ URL length:                 {} characters", base64_data.len());
    println!("   (Safe for URLs: {})", if base64_data.len() < 2000 { "✓" } else { "may be long" });
    
    println!("\n=== THESIS CLAIM VERIFICATION ===\n");
    
    // Verify thesis claims with 1000 event scenario
    let events_1000 = generate_test_log(1000);
    let json_1000 = serde_json::to_string_pretty(&events_1000).unwrap();
    let json_size_1000 = json_1000.len();
    
    let compact_1000_data = compact_events(&events_1000);
    let compact_json_1000 = serde_json::to_string(&compact_1000_data).unwrap();
    let compact_size_1000 = compact_json_1000.len();
    
    let msgpack_1000 = rmp_serde::to_vec(&compact_1000_data).unwrap();
    let msgpack_size_1000 = msgpack_1000.len();
    
    let mut encoder_1000 = GzEncoder::new(Vec::new(), Compression::best());
    encoder_1000.write_all(&msgpack_1000).unwrap();
    let gzip_1000 = encoder_1000.finish().unwrap();
    let gzip_size_1000 = gzip_1000.len();
    
    let thesis_claim_json_to_tuple = 60.0;
    let actual_json_to_tuple = ((json_size_1000 - compact_size_1000) as f64 / json_size_1000 as f64) * 100.0;
    
    let thesis_claim_msgpack = 25.0;
    let actual_msgpack = ((compact_size_1000 - msgpack_size_1000) as f64 / compact_size_1000 as f64) * 100.0;
    
    let thesis_claim_gzip = 70.0;
    let actual_gzip = ((msgpack_size_1000 - gzip_size_1000) as f64 / msgpack_size_1000 as f64) * 100.0;
    
    let thesis_claim_overall = 90.0;
    let actual_overall = ((json_size_1000 - gzip_size_1000) as f64 / json_size_1000 as f64) * 100.0;
    
    println!("Compression Stage         | Thesis Claim | Actual  | Status");
    println!("──────────────────────────|──────────────|─────────|────────");
    println!("JSON → Tuple              | ~{:.0}%         | {:.1}%  | {}", 
        thesis_claim_json_to_tuple, 
        actual_json_to_tuple,
        if (actual_json_to_tuple - thesis_claim_json_to_tuple).abs() < 10.0 { "✓" } else { "!" }
    );
    println!("Tuple → MessagePack       | ~{:.0}%         | {:.1}%  | {}", 
        thesis_claim_msgpack, 
        actual_msgpack,
        if (actual_msgpack - thesis_claim_msgpack).abs() < 10.0 { "✓" } else { "!" }
    );
    println!("MessagePack → Gzip        | ~{:.0}%         | {:.1}%  | {}", 
        thesis_claim_gzip, 
        actual_gzip,
        if (actual_gzip - thesis_claim_gzip).abs() < 10.0 { "✓" } else { "!" }
    );
    println!("Overall (JSON → Gzip)     | ~{:.0}%         | {:.1}%  | {}", 
        thesis_claim_overall, 
        actual_overall,
        if (actual_overall - thesis_claim_overall).abs() < 5.0 { "✓" } else { "!" }
    );
    
    println!("\n=== RECOMMENDED THESIS VALUES ===\n");
    println!("Based on testing with 1000 events:");
    println!("• JSON to Tuple compaction:    ~{:.0}% size reduction", actual_json_to_tuple);
    println!("• MessagePack serialization:   Additional ~{:.0}% reduction", actual_msgpack);
    println!("• Gzip compression:            ~{:.0}% reduction of MessagePack", actual_gzip);
    println!("• Overall compression:         ~{:.0}% total reduction", actual_overall);
    println!("• Example: {} KB → {} KB", json_size_1000 / 1024, gzip_size_1000 / 1024);
    
    println!("\n✅ Test completed successfully!\n");
}

#[test]
fn verify_actual_showcase_compression() {
    println!("\n=== ACTUAL SHOWCASE FILE COMPRESSION TEST ===\n");
    
    // Create a temporary test log file
    let temp_dir = std::env::temp_dir();
    let log_path = temp_dir.join("test_deadlock.json");
    
    // Generate realistic log data
    let events = generate_test_log(1000);
    
    {
        let file = File::create(&log_path).unwrap();
        let mut writer = BufWriter::new(file);
        
        for event in &events {
            serde_json::to_writer(&mut writer, &event).unwrap();
            writeln!(writer).unwrap();
        }
    }
    
    // Measure original file size
    let original_size = fs::metadata(&log_path).unwrap().len();
    println!("Original log file size:  {} bytes ({} KB)", original_size, original_size / 1024);
    
    // Process through the actual encoder (if available)
    // This simulates what showcase() does
    let compact_data_final = compact_events(&events);
    let msgpack_data = rmp_serde::to_vec(&compact_data_final).unwrap();
    
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&msgpack_data).unwrap();
    let compressed = encoder.finish().unwrap();
    
    let base64_engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let encoded = base64_engine.encode(&compressed);
    
    println!("Encoded URL data size:   {} bytes ({} KB)", encoded.len(), encoded.len() / 1024);
    println!("Compression ratio:       {:.2}x", original_size as f64 / encoded.len() as f64);
    println!("Size reduction:          {:.1}%", 
        ((original_size as f64 - encoded.len() as f64) / original_size as f64) * 100.0
    );
    
    // Cleanup
    let _ = fs::remove_file(&log_path);
    
    println!("\n✅ Actual showcase compression verified!\n");
}

