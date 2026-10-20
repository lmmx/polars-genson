//! An integer beyond the 64-bit range is a number, as serde_json (which validates and
//! normalises the input) reads it, rather than a parse error that panicked in the build.

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

fn type_of_a(document: &str) -> serde_json::Value {
    let result =
        infer_json_schema_from_strings(&[document.to_string()], SchemaInferenceConfig::default())
            .unwrap();
    result.schema["properties"]["a"]["type"].clone()
}

#[test]
fn test_integer_beyond_64_bits_is_a_number() {
    assert_eq!(
        type_of_a(r#"{"a": 123456789012345678901234567890}"#),
        json!("number")
    );
    assert_eq!(
        type_of_a(r#"{"a": -123456789012345678901234567890}"#),
        json!("number")
    );
}

#[test]
fn test_integers_at_the_signed_64_bit_limits_stay_integers() {
    assert_eq!(type_of_a(r#"{"a": 9223372036854775807}"#), json!("integer"));
    assert_eq!(
        type_of_a(r#"{"a": -9223372036854775808}"#),
        json!("integer")
    );
}

/// An integer above `i64::MAX` (up to `u64::MAX`) is a number too: no Avro `long` or
/// Arrow `Int64` holds it, and as an integer it normalised to null.
#[test]
fn test_integer_above_the_signed_64_bit_range_is_a_number() {
    for a in ["9223372036854775808", "18446744073709551615"] {
        assert_eq!(
            type_of_a(&format!(r#"{{"a": {a}}}"#)),
            json!("number"),
            "{a}"
        );
    }
    let mut rows: Vec<String> = (0..12).map(|i| format!(r#"{{"a": {i}}}"#)).collect();
    rows.push(r#"{"a": 18446744073709551615}"#.to_string());
    let result = infer_json_schema_from_strings(&rows, SchemaInferenceConfig::default()).unwrap();
    assert_eq!(result.schema["properties"]["a"]["type"], json!("number"));
}

#[cfg(feature = "avro")]
#[test]
fn test_integer_above_the_signed_64_bit_range_is_kept_by_normalisation() {
    use genson_core::normalise::{normalise_values, NormaliseConfig};
    let rows = [r#"{"a": 18446744073709551615}"#, r#"{"a": 1}"#];
    let config = SchemaInferenceConfig {
        avro: true,
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(&rows, config)
        .unwrap()
        .schema;
    let values = rows
        .iter()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    let out = normalise_values(values, &schema, &NormaliseConfig::default());
    assert_eq!(out[0]["a"].as_f64(), Some(18446744073709551615.0));
    assert_eq!(out[1]["a"].as_f64(), Some(1.0));
}

#[test]
fn test_integer_beyond_64_bits_among_rows() {
    // 10 or more strings take the parallel path, which hashes each row's builder
    let mut rows: Vec<String> = (0..12).map(|i| format!(r#"{{"a": {i}}}"#)).collect();
    rows.push(r#"{"a": 99999999999999999999999}"#.to_string());
    let result = infer_json_schema_from_strings(&rows, SchemaInferenceConfig::default()).unwrap();
    assert_eq!(result.schema["properties"]["a"]["type"], json!("number"));
}
