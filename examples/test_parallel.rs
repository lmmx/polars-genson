#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! genson-core = { path = "../genson-core", features = ["avro"] }
//! serde_json = "1.0"
//! ```

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::Duration;
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use genson_core::normalise::{normalise_values, NormaliseConfig, MapEncoding};

/// Get current RSS memory usage in bytes
fn get_rss_bytes() -> Option<usize> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: usize = parts[1].parse().ok()?;
                return Some(kb * 1024); // Convert KB to bytes
            }
        }
    }
    None
}

/// Format bytes to human-readable string
fn format_bytes(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    const GB: usize = 1024 * 1024 * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    }
}

struct MemoryTracker {
    start_rss: usize,
    peak_rss: Arc<AtomicUsize>,
    monitoring: Arc<std::sync::atomic::AtomicBool>,
    monitor_thread: Option<thread::JoinHandle<()>>,
}

impl MemoryTracker {
    fn new() -> Self {
        // Sleep briefly to let the process stabilize
        thread::sleep(Duration::from_millis(100));
        let start_rss = get_rss_bytes().unwrap_or(0);
        println!("📊 Memory tracking started");
        println!("   Start RSS: {}", format_bytes(start_rss));
        
        let peak_rss = Arc::new(AtomicUsize::new(start_rss));
        let monitoring = Arc::new(std::sync::atomic::AtomicBool::new(true));
        
        // Spawn a background thread to continuously monitor memory
        let peak_clone = Arc::clone(&peak_rss);
        let monitoring_clone = Arc::clone(&monitoring);
        let monitor_thread = thread::spawn(move || {
            while monitoring_clone.load(Ordering::Relaxed) {
                if let Some(current) = get_rss_bytes() {
                    peak_clone.fetch_max(current, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(10)); // Check every 10ms
            }
        });
        
        Self {
            start_rss,
            peak_rss,
            monitoring,
            monitor_thread: Some(monitor_thread),
        }
    }

    fn report_final(mut self) {
        // Stop monitoring
        self.monitoring.store(false, Ordering::Relaxed);
        if let Some(handle) = self.monitor_thread.take() {
            handle.join().ok();
        }
        
        let end_rss = get_rss_bytes().unwrap_or(0);
        let peak_rss = self.peak_rss.load(Ordering::Relaxed);
        
        println!("\n📊 Memory Usage Summary:");
        println!("   Start RSS: {}", format_bytes(self.start_rss));
        println!("   Peak RSS:  {}", format_bytes(peak_rss));
        println!("   End RSS:   {}", format_bytes(end_rss));
        println!("   Delta:     {}", format_bytes(end_rss.saturating_sub(self.start_rss)));
    }
}

fn main() {
    let mem_tracker = MemoryTracker::new();
    
    // Show current RSS after initialization
    if let Some(current) = get_rss_bytes() {
        println!("   Current RSS after init: {}", format_bytes(current));
    }

    // Simulate what the Python extension does with 30 rows
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    let path = PathBuf::from(home)
        .join("dev/polars-genson/genson-cli/tests/data/claims_fixture_x30.jsonl");
    let json_strings: Vec<String> = fs::read_to_string(&path)
        .expect("Failed to read JSONL file")
        .lines()
        // .take(5)
        // .cycle()
        .take(30)
        .map(String::from)
        .collect();

    println!("\nTesting with {} JSON strings", json_strings.len());
    if let Some(current) = get_rss_bytes() {
        println!("Current RSS after loading data: {}", format_bytes(current));
    }

    // FIRST PASS: Infer schema from original JSON
    let config = SchemaInferenceConfig {
        ignore_outer_array: true,
        delimiter: None,
        schema_uri: Some("AUTO".to_string()),
        map_threshold: 0,
        map_max_required_keys: None,
        unify_maps: true,
        no_unify: std::collections::HashSet::new(),
        force_field_types: std::collections::HashMap::new(),
        wrap_scalars: true,
        avro: true,
        wrap_root: Some("claims".to_string()),
        no_root_map: true,
        max_builders: None,
        debug: false,
        profile: true,
        verbosity: genson_core::DebugVerbosity::Normal,
    };

    println!("\n=== FIRST INFERENCE (original JSON) ===");
    let schema_result = infer_json_schema_from_strings(&json_strings, config.clone())
        .expect("Schema inference failed");

    println!("Schema inferred, {} objects processed", schema_result.processed_count);
    if let Some(current) = get_rss_bytes() {
        println!("Current RSS after first inference: {}", format_bytes(current));
    }

    // NORMALIZATION PASS
    println!("\n=== NORMALIZATION ===");
    let norm_config = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root: Some("claims".to_string()),
    };

    let mut normalized_jsons = Vec::new();
    for json_str in &json_strings {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .expect("Failed to parse JSON");
        let normed = normalise_values(vec![val], &schema_result.schema, &norm_config)
            .pop()
            .unwrap();
        normalized_jsons.push(serde_json::to_string(&normed).unwrap());
    }

    println!("Normalized {} JSON strings", normalized_jsons.len());

    // SECOND PASS: Infer schema from normalized JSON (for decode=True)
    println!("\n=== SECOND INFERENCE (normalized JSON) ===");
    let schema_result2 = infer_json_schema_from_strings(&normalized_jsons, config)
        .expect("Second schema inference failed");

    println!("Second schema inferred, {} objects processed", schema_result2.processed_count);
    
    println!("\n=== COMPLETE ===");
    mem_tracker.report_final();
}
