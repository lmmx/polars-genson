#![cfg(feature = "avro")]
//! An object empty in every row stays an Avro record with no fields (`Struct({})`).
//! avrotize (from 0.2.0) reads an object schema without properties of any kind as holding
//! any properties, a map of its generic union; genson gives such objects `"properties":
//! {}` first.

use genson_core::normalise::{normalise_values, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

fn avro(rows: &[&str]) -> Value {
    let config = SchemaInferenceConfig {
        avro: true,
        ..SchemaInferenceConfig::default()
    };
    infer_json_schema_from_strings(rows, config).unwrap().schema
}

fn is_empty_record(schema: &Value) -> bool {
    schema["type"] == "record" && schema["fields"] == json!([])
}

#[test]
fn test_always_empty_object_is_a_record_with_no_fields() {
    let fields = avro(&[r#"{"a": {}}"#, r#"{"a": {}}"#])["fields"].clone();
    assert!(is_empty_record(&fields[0]["type"]), "{fields}");
}

/// Empty in one row and null in another: a union of null and the empty record.
#[test]
fn test_nullable_always_empty_object_is_a_record_with_no_fields() {
    let fields = avro(&[r#"{"a": {}, "b": 1}"#, r#"{"a": null, "b": 2}"#])["fields"].clone();
    let branches = fields[0]["type"].as_array().expect("a union").clone();
    assert!(branches.contains(&json!("null")), "{fields}");
    assert!(branches.iter().any(is_empty_record), "{fields}");
    assert!(!branches.iter().any(|b| b["type"] == "map"), "{fields}");
}

#[test]
fn test_list_of_always_empty_objects_holds_records_with_no_fields() {
    let fields = avro(&[r#"{"a": [{}]}"#, r#"{"a": [{}, {}]}"#])["fields"].clone();
    assert_eq!(fields[0]["type"]["type"], "array", "{fields}");
    assert!(is_empty_record(&fields[0]["type"]["items"]), "{fields}");
}

#[test]
fn test_always_empty_object_in_a_record_is_a_record_with_no_fields() {
    let fields = avro(&[r#"{"a": {"x": {}, "y": 1}}"#])["fields"].clone();
    let inner = &fields[0]["type"]["fields"];
    assert_eq!(inner[0]["name"], "x", "{fields}");
    assert!(is_empty_record(&inner[0]["type"]), "{fields}");
    assert_eq!(inner[1]["type"], "int", "{fields}");
}

/// Objects with keys, and maps (here every object is a map candidate), are untouched.
#[test]
fn test_objects_with_keys_and_maps_are_unchanged() {
    let fields = avro(&[r#"{"a": {"x": 1}}"#])["fields"].clone();
    assert_eq!(fields[0]["type"]["fields"][0]["type"], "int", "{fields}");

    let config = SchemaInferenceConfig {
        avro: true,
        map_threshold: 0,
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(&[r#"{"a": {}}"#, r#"{"a": {}}"#], config)
        .unwrap()
        .schema;
    let a = &schema["fields"][0]["type"];
    assert_eq!(a["type"], "map", "{schema}");
    assert_eq!(a["values"], "null", "{schema}");
}

/// Normalisation keeps an always-empty object as an empty object.
#[test]
fn test_always_empty_object_normalises_to_an_empty_object() {
    let rows = [r#"{"a": {}, "b": 1}"#, r#"{"a": {}, "b": 2}"#];
    let schema = avro(&rows);
    let values: Vec<Value> = rows
        .iter()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    let cfg = NormaliseConfig {
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    let normalised = normalise_values(values.clone(), &schema, &cfg);
    assert_eq!(normalised, values);
}
