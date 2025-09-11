// genson-core/tests/map_max_required_keys.rs

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

#[test]
fn test_map_max_required_keys_none_preserves_existing_behavior() {
    // With map_max_required_keys = None, should behave exactly as before
    let json_strings =
        vec![r#"{"labels": {"en": "Hello", "fr": "Bonjour", "de": "Hallo"}}"#.to_string()];

    let config = SchemaInferenceConfig {
        map_threshold: 2,            // Low threshold to trigger map inference
        map_max_required_keys: None, // Should not gate based on required keys
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    let labels = &result.schema["properties"]["labels"];
    assert_eq!(labels["type"], "object");
    assert!(labels.get("additionalProperties").is_some());
    assert!(labels.get("properties").is_none());
    println!("✅ None value preserves existing map inference behavior");
}

#[test]
fn test_map_max_required_keys_blocks_high_required_count() {
    // All keys are required, should be blocked from map inference
    let json_strings = vec![
        r#"{"user_id": 1, "attrs": {"source": "web", "campaign": "abc"}}"#.to_string(),
        r#"{"user_id": 2, "attrs": {"source": "mobile", "campaign": "def"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 2,               // Low threshold to allow map candidates
        map_max_required_keys: Some(1), // Max 1 required key for maps
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // Top level: user_id and attrs are both required (2 > 1) → should be Record
    let root = &result.schema;
    assert_eq!(root["type"], "object");
    assert!(root.get("properties").is_some());
    assert!(root.get("additionalProperties").is_none());

    // attrs: source and campaign are both required (2 > 1) → should be Record
    let attrs = &root["properties"]["attrs"];
    assert_eq!(attrs["type"], "object");
    assert!(attrs.get("properties").is_some());
    assert!(attrs.get("additionalProperties").is_none());

    println!("✅ High required key count blocked from map inference");
}

#[test]
fn test_map_max_required_keys_allows_low_required_count() {
    // Mix of required and optional keys
    let json_strings = vec![
        r#"{"user_id": 1, "attrs": {"source": "web", "campaign": "abc"}}"#.to_string(),
        r#"{"user_id": 2, "attrs": {"source": "mobile"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 2,               // Low threshold to allow map candidates
        map_max_required_keys: Some(1), // Max 1 required key for maps
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // Top level: user_id and attrs are both required (2 > 1) → should be Record
    let root = &result.schema;
    assert_eq!(root["type"], "object");
    assert!(root.get("properties").is_some());
    assert!(root.get("additionalProperties").is_none());

    // attrs: only "source" is required, "campaign" is optional (1 ≤ 1) → should be Map
    let attrs = &root["properties"]["attrs"];
    assert_eq!(attrs["type"], "object");
    assert!(attrs.get("additionalProperties").is_some());
    assert!(attrs.get("properties").is_none());

    println!("✅ Low required key count allowed for map inference");
}

#[test]
fn test_map_max_required_keys_zero_requires_no_required_keys() {
    // Only objects with zero required keys can be maps
    let json_strings = vec![
        r#"{"metadata": {"key1": "value1", "key2": "value2"}}"#.to_string(),
        r#"{"metadata": {"key3": "value3"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(0), // Only allow maps with 0 required keys
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // metadata: no keys are required (all are optional) → should be Map
    let metadata = &result.schema["properties"]["metadata"];
    assert_eq!(metadata["type"], "object");
    assert!(metadata.get("additionalProperties").is_some());
    assert!(metadata.get("properties").is_none());

    println!("✅ Zero threshold allows only fully optional key maps");
}

#[test]
fn test_map_max_required_keys_with_force_override() {
    // Force override should take precedence over required key gating
    let json_strings = vec![
        r#"{"labels": {"en": "Hello", "fr": "Bonjour"}}"#.to_string(),
        r#"{"labels": {"en": "World", "fr": "Monde"}}"#.to_string(),
    ];

    let mut force_types = std::collections::HashMap::new();
    force_types.insert("labels".to_string(), "map".to_string());

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(0), // Would normally block this
        force_field_types: force_types,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // labels: forced to map despite having required keys
    let labels = &result.schema["properties"]["labels"];
    assert_eq!(labels["type"], "object");
    assert!(labels.get("additionalProperties").is_some());
    assert!(labels.get("properties").is_none());

    println!("✅ Force override takes precedence over required key gating");
}

#[test]
fn test_complex_nested_example() {
    // Real-world example: user records with attribute maps
    let json_strings = vec![
        json!({
            "user_id": 123,
            "profile": { "name": "Alice", "age": 30 },
            "attributes": { "source": "web", "campaign": "summer2024" }
        })
        .to_string(),
        json!({
            "user_id": 456,
            "profile": { "name": "Bob", "age": 25 },
            "attributes": { "source": "mobile", "utm_medium": "social" }
        })
        .to_string(),
        json!({
            "user_id": 789,
            "profile": { "name": "Charlie", "age": 35 },
            "attributes": { "source": "email" }
        })
        .to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(1), // Allow maps with ≤1 required key
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    let schema_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Complex nested schema:\n{}", schema_str);

    // Root level: user_id, profile, attributes all required (3 > 1) → Record
    assert_eq!(result.schema["type"], "object");
    assert!(result.schema.get("properties").is_some());
    assert!(result.schema.get("additionalProperties").is_none());

    // profile: name and age both required (2 > 1) → Record
    let profile = &result.schema["properties"]["profile"];
    assert_eq!(profile["type"], "object");
    assert!(profile.get("properties").is_some());
    assert!(profile.get("additionalProperties").is_none());

    // attributes: only "source" is required, others optional (1 ≤ 1) → Map
    let attributes = &result.schema["properties"]["attributes"];
    assert_eq!(attributes["type"], "object");
    assert!(attributes.get("additionalProperties").is_some());
    assert!(attributes.get("properties").is_none());

    println!("✅ Complex nested example correctly discriminates records vs maps");
}
