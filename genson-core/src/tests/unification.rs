use super::*;
use serde_json::json;
use crate::infer_json_schema_from_strings;

#[test]
fn test_scalar_unification_ndjson_mixed_nullable_formats() {
    let ndjson_input = r#"
{"theme": {"red": {"colors": {"primary": "ff"}}, "blue": {"colors": {"secondary": "00"}}}}
{"theme": {"green": {"colors": {"accent": "cc"}}, "red": {"colors": {"secondary": "aa"}}}}
{"theme": {"blue": {"brightness": 255}}}
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
    println!("Generated schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    // Navigate to the colors field that should have been unified
    let themes_schema = &result.schema["properties"]["theme"];
    assert!(themes_schema.get("additionalProperties").is_some());

    let theme_record = &themes_schema["additionalProperties"];
    let colors_schema = &theme_record["properties"]["colors"];

    // Should be converted to map due to scalar unification
    assert!(colors_schema.get("additionalProperties").is_some());
    assert!(colors_schema.get("properties").is_none());

    // The unified type should be nullable string (some colors components missing from some themes)
    let colors_values = &colors_schema["additionalProperties"];
    assert_eq!(colors_values["type"], json!(["null", "string"]));
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

#[ignore]
#[test]
fn test_check_unifiable_schemas_anyof_case() {
    let schemas = vec![
        json!({"type": "object", "properties": {"timezone": {"type": "integer"}}}),
        json!({"type": "string"})
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = check_unifiable_schemas(&schemas, "datavalue", &config);
    println!("Generated schema: {}", serde_json::to_string_pretty(&result).unwrap());

    // Currently returns None, should return unified schema with scalar promotion
    assert!(result.is_some(), "Should unify mixed scalar+object schemas with wrap_scalars");
}

#[ignore]
#[test]
fn test_scalar_vs_mixed_type_object_unification() {
    let test_data = vec![
        json!({"datavalue": "7139c051-8ea3-3f93-8bbc-6e7dff6d61a4"}).to_string(),
        json!({"datavalue": {"timezone": 0, "precision": 11}}).to_string(),
        json!({"datavalue": {"id": "Q1022293", "labels": {"ru": "до мажор"}}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config)
        .expect("Should succeed with scalar promotion and record unification");
    println!("Generated schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["datavalue"];
    assert_eq!(datavalue_schema["type"], "object");

    // Should have properties (record) not additionalProperties (map) due to mixed value types
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema.get("additionalProperties").is_none());
}
