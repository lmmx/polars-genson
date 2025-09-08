#![cfg(feature = "avro")]

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};

#[test]
fn test_simple_object_to_avro() {
    let jsons = vec![
        r#"{"name": "Alice", "age": 30}"#.to_string(),
        r#"{"name": "Bob", "age": 25}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        avro: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&jsons, config)
        .expect("Schema inference with Avro output should succeed");

    let avro_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Avro schema:\n{}", avro_str);

    assert!(avro_str.contains(r#""type": "record""#));
    assert!(avro_str.contains(r#""name": "document""#));
    assert!(avro_str.contains(r#""name": "name""#));
    assert!(avro_str.contains(r#""name": "age""#));
}

#[test]
fn test_map_field_to_avro() {
    // keys vary but values are homogeneous strings
    let jsons = vec![
        r#"{"labels": {"en": "Hello", "fr": "Bonjour"}}"#.to_string(),
        r#"{"labels": {"de": "Hallo", "es": "Hola"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        avro: true,
        // force a low threshold so it rewrites as map quickly
        map_threshold: 2,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&jsons, config)
        .expect("Schema inference with Avro output should succeed");

    let avro_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Avro schema with map field:\n{}", avro_str);

    assert!(avro_str.contains(r#""name": "labels""#));
    assert!(
        avro_str.contains(r#""type": "map""#),
        "Expected labels field to become an Avro map"
    );
    assert!(
        avro_str.contains(r#""values": "string""#),
        "Avro map should have string values"
    );
}
