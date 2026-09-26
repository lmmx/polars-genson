use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// The value schema of `m` when `m` is forced to be a map.
fn forced_map_values(rows: &[&str]) -> Value {
    let config = SchemaInferenceConfig {
        force_field_types: [("m".to_string(), "map".to_string())].into(),
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(rows, config).unwrap().schema;
    let m = &schema["properties"]["m"];
    assert!(m.get("properties").is_none(), "m should be a map: {m}");
    m["additionalProperties"].clone()
}

/// A forced map keeps its values' type rather than turning them into strings.
#[test]
fn test_forced_map_keeps_integer_values() {
    let values = forced_map_values(&[r#"{"m": {"a": 1, "b": 2}}"#, r#"{"m": {"c": 3}}"#]);
    assert_eq!(values["type"], json!("integer"));
}

#[test]
fn test_forced_map_keeps_string_values() {
    let values = forced_map_values(&[r#"{"m": {"en": "hi", "fr": "salut"}}"#]);
    assert_eq!(values["type"], json!("string"));
}

#[test]
fn test_forced_map_of_records_keeps_the_record() {
    let values = forced_map_values(&[r#"{"m": {"a": {"n": 1}, "b": {"n": 2}}}"#]);
    assert_eq!(values["type"], json!("object"));
    assert_eq!(values["properties"]["n"]["type"], json!("integer"));
}

/// Records with different fields unify into one record type with every field.
#[test]
fn test_forced_map_unifies_differing_records() {
    let values = forced_map_values(&[r#"{"m": {"a": {"n": 1}, "b": {"s": "x"}}}"#]);
    let props = values["properties"].as_object().unwrap();
    assert!(
        props.contains_key("n") && props.contains_key("s"),
        "{values}"
    );
}

/// Values with no common type fall back to strings, so no value is lost.
#[test]
fn test_forced_map_with_mixed_scalar_values_falls_back_to_string() {
    let values = forced_map_values(&[r#"{"m": {"a": 1, "b": "x"}}"#]);
    assert_eq!(values["type"], json!("string"));
}
