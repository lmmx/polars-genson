#![cfg(feature = "avro")]

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

fn avro_fields(rows: &[&str]) -> serde_json::Value {
    let config = SchemaInferenceConfig {
        avro: true,
        ..SchemaInferenceConfig::default()
    };
    infer_json_schema_from_strings(rows, config).unwrap().schema["fields"].clone()
}

/// An array that is empty in every row gets null items, not avrotize's generic union.
#[test]
fn test_always_empty_array_items_are_null() {
    let fields = avro_fields(&[r#"{"tags": []}"#, r#"{"tags": []}"#]);
    assert_eq!(fields[0]["type"], json!({"type": "array", "items": "null"}));
}

/// An array that is empty in some rows and missing or null in others keeps null items.
#[test]
fn test_nullable_always_empty_array_items_are_null() {
    let fields = avro_fields(&[r#"{"tags": []}"#, r#"{"tags": null}"#]);
    assert_eq!(
        fields[0]["type"],
        json!([{"type": "array", "items": "null"}, "null"])
    );
}

/// Arrays with values keep their item type.
#[test]
fn test_non_empty_array_items_unchanged() {
    let fields = avro_fields(&[r#"{"tags": []}"#, r#"{"tags": ["a"]}"#]);
    assert_eq!(
        fields[0]["type"],
        json!({"type": "array", "items": "string"})
    );
}
