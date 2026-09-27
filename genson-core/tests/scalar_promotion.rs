#![cfg(feature = "avro")]

use genson_core::normalise::{normalise_values, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

fn infer_and_normalise(rows: &[&str], config: SchemaInferenceConfig) -> Vec<Value> {
    let config = SchemaInferenceConfig {
        avro: true,
        ..config
    };
    let schema = infer_json_schema_from_strings(rows, config).unwrap().schema;
    let values = rows
        .iter()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    normalise_values(values, &schema, &NormaliseConfig::default())
}

/// A field that is a string in one row and an object in another keeps the string
/// (as `value__string`) without needing unify_maps.
#[test]
fn test_scalar_object_union_promoted_without_unify_maps() {
    let rows = [
        r#"{"value": "plain"}"#,
        r#"{"value": {"amount": 3, "unit": "kg"}}"#,
    ];
    let config = SchemaInferenceConfig::default();
    assert!(!config.unify_maps && config.wrap_scalars);

    let out = infer_and_normalise(&rows, config);
    assert_eq!(out[0]["value"]["value__string"], json!("plain"));
    assert_eq!(out[1]["value"]["amount"], json!(3));
    assert_eq!(out[1]["value"]["value__string"], json!(null));
}

/// With wrap_scalars off, the union is left alone (no promoted field is added).
#[test]
fn test_scalar_object_union_untouched_without_wrap_scalars() {
    let rows = [
        r#"{"value": "plain"}"#,
        r#"{"value": {"amount": 3, "unit": "kg"}}"#,
    ];
    let config = SchemaInferenceConfig {
        wrap_scalars: false,
        ..SchemaInferenceConfig::default()
    };
    let out = infer_and_normalise(&rows, config);
    assert!(out[1]["value"].get("value__string").is_none());
}

/// Rows of a map whose values are lists of records, one of which holds a bare string
/// instead of a record (a Wikidata reference to a deleted property): the string is
/// promoted among the records (under the map's field name), rather than making the map
/// fall back to a record.
fn map_of_record_lists_with_scalar_item() -> ([&'static str; 3], SchemaInferenceConfig) {
    let rows = [
        r#"{"refs": {"P248": [{"property": "P248", "datavalue": {"id": "Q1"}}]}}"#,
        r#"{"refs": {"P248": [{"property": "P248", "datavalue": {"id": "Q2"}}], "P813": [{"property": "P813", "datavalue": {"id": "Q3"}}]}}"#,
        r#"{"refs": {"P4003": ["P4003"]}}"#,
    ];
    let config = SchemaInferenceConfig {
        map_threshold: 0,
        unify_maps: true,
        ..SchemaInferenceConfig::default()
    };
    (rows, config)
}

#[test]
fn test_scalar_item_among_record_items_promoted() {
    let (rows, config) = map_of_record_lists_with_scalar_item();
    let out = infer_and_normalise(&rows, config);
    assert_eq!(out[1]["refs"]["P813"][0]["property"], json!("P813"));
    assert_eq!(out[1]["refs"]["P813"][0]["refs__string"], json!(null));
    assert_eq!(out[2]["refs"]["P4003"][0]["refs__string"], json!("P4003"));
    assert_eq!(out[2]["refs"]["P4003"][0]["property"], json!(null));
}

/// With wrap_scalars off, the lists can't be unified, so the map stays a record.
#[test]
fn test_scalar_item_among_record_items_untouched_without_wrap_scalars() {
    let (rows, config) = map_of_record_lists_with_scalar_item();
    let config = SchemaInferenceConfig {
        wrap_scalars: false,
        avro: true,
        ..config
    };
    let schema = infer_json_schema_from_strings(&rows, config)
        .unwrap()
        .schema;
    let refs = &schema["fields"][0]["type"];
    assert_eq!(refs["type"], json!("record"), "{refs}");
}
