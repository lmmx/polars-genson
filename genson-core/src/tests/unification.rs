use super::*;
use serde_json::json;
use crate::infer_json_schema_from_strings;

#[test]
fn test_scalar_unification_ndjson_mixed_nullable_formats() {
    let ndjson_input = r#"
{"colors": {"a": {"hex": {"r": "ff"}}, "b": {"hex": {"g": "00"}}}}
{"colors": {"c": {"hex": {"b": "cc"}}, "a": {"hex": {"g": "aa"}}}}
{"colors": {"b": {"rgb": 255}}}
"#;

    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 1,
        unify_maps: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&[ndjson_input.to_string()], config)
        .expect("Should handle NDJSON with nested maps triggering scalar unification");

    // Navigate to the hex field that should have been unified
    let colors_schema = &result.schema["properties"]["colors"];
    assert!(colors_schema.get("additionalProperties").is_some());

    let color_record = &colors_schema["additionalProperties"];
    let hex_schema = &color_record["properties"]["hex"];

    // Should be converted to map due to scalar unification
    assert!(hex_schema.get("additionalProperties").is_some());
    assert!(hex_schema.get("properties").is_none());

    // The unified type should be nullable string (some hex components missing from some colors)
    let hex_values = &hex_schema["additionalProperties"];
    assert_eq!(hex_values["type"], json!(["null", "string"]));
}

#[test]
fn test_scalar_unification_with_old_nullable_format() {
    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        ..Default::default()
    };

    // Simulate the old nullable format that was causing issues
    let schemas = vec![
        json!({"type": "string"}),                           // Regular string
        json!({"type": ["null", "string"]}),                 // New nullable format
        json!(["null", {"type": ["null", "string"]}]),       // Old nullable format
    ];

    let result = check_unifiable_schemas(&schemas, "test", &config);
    
    // Should successfully unify all scalar string types
    assert!(result.is_some());
    let unified = result.unwrap();
    assert_eq!(unified["type"], json!(["null", "string"]));
}

#[test]
fn test_is_scalar_schema_with_mixed_formats() {
    // Test the updated is_scalar_schema function
    assert!(is_scalar_schema(&json!({"type": "string"})));
    assert!(is_scalar_schema(&json!({"type": ["null", "string"]})));
    assert!(is_scalar_schema(&json!(["null", {"type": "string"}])));
    assert!(is_scalar_schema(&json!(["null", {"type": ["null", "string"]}])));
    
    // Should reject object types
    assert!(!is_scalar_schema(&json!({"type": "object", "properties": {}})));
    assert!(!is_scalar_schema(&json!({"type": "array", "items": {}})));
}
