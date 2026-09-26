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
