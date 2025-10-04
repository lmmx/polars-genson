#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! genson-core = { path = "../genson-core", features = ["avro"] }
//! serde_json = "1.0"
//! ```

use std::fs;
use std::path::PathBuf;
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use genson_core::normalise::{normalise_values, NormaliseConfig, MapEncoding};

fn main() {
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

    println!("Testing with {} JSON strings", json_strings.len());

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
        debug: false,
        profile: true,
        verbosity: genson_core::DebugVerbosity::Normal,
    };

    println!("\n=== FIRST INFERENCE (original JSON) ===");
    let schema_result = infer_json_schema_from_strings(&json_strings, config.clone())
        .expect("Schema inference failed");

    println!("Schema inferred, {} objects processed", schema_result.processed_count);

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
}
