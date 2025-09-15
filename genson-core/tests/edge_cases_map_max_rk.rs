// genson-core/tests/edge_cases_map_max_rk.rs

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

#[test]
fn test_empty_objects_handled_correctly() {
    let json_strings = vec![r#"{"empty": {}}"#.to_string()];

    let config = SchemaInferenceConfig {
        map_threshold: 0, // Even empty objects are candidates
        map_max_required_keys: Some(0),
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // empty object has 0 keys and 0 required keys, should be allowed as map
    let empty_field = &result.schema["properties"]["empty"];
    // But since it has 0 keys, it won't meet map_threshold anyway
    // This tests that our logic doesn't crash on edge cases
    assert!(empty_field.is_object());

    println!("✅ Empty objects handled correctly");
}

#[test]
fn test_single_key_objects() {
    let json_strings = vec![
        r#"{"single": {"one_key": "value"}}"#.to_string(),
        r#"{"single": {"extra_key": "value"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        map_max_required_keys: Some(0), // Exactly at the threshold
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // single has 1 key which is required, should be allowed (1 ≤ 1)
    let single_field = &result.schema["properties"]["single"];
    assert_eq!(single_field["type"], "object");
    assert!(single_field.get("additionalProperties").is_some());

    println!("✅ Single key objects at threshold boundary work correctly");
}

#[test]
fn test_mixed_schemas_with_varying_required_counts() {
    // Complex scenario with nested objects having different required key patterns
    let json_strings = vec![
        json!({
            "always_present": {
                "req1": "val1",
                "req2": "val2"
            },
            "sometimes_present": {
                "common": "always here",
                "rare": "sometimes here"
            },
            "never_consistent": {
                "key1": "val1"
            }
        })
        .to_string(),
        json!({
            "always_present": {
                "req1": "val3",
                "req2": "val4"
            },
            "sometimes_present": {
                "common": "still here"
            },
            "never_consistent": {
                "key2": "val2"
            }
        })
        .to_string(),
        json!({
            "always_present": {
                "req1": "val5",
                "req2": "val6"
            },
            "sometimes_present": {
                "common": "always present",
                "other": "different key"
            },
            "never_consistent": {
                "key3": "val3"
            }
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
    println!("Mixed required counts schema:\n{}", schema_str);

    // always_present: req1, req2 always required (2 > 1) → Record
    let always = &result.schema["properties"]["always_present"];
    assert!(always.get("properties").is_some());
    assert!(always.get("additionalProperties").is_none());

    // sometimes_present: only "common" is required (1 ≤ 1) → Map
    let sometimes = &result.schema["properties"]["sometimes_present"];
    assert!(sometimes.get("additionalProperties").is_some());
    assert!(sometimes.get("properties").is_none());

    // never_consistent: no keys are required (0 ≤ 1) → Map
    let never = &result.schema["properties"]["never_consistent"];
    assert!(never.get("additionalProperties").is_some());
    assert!(never.get("properties").is_none());

    println!("✅ Mixed required key patterns handled correctly");
}

#[test]
fn test_very_high_max_required_keys() {
    // Test with very high threshold - should behave like None
    let json_strings = vec![
        r#"{"many_required": {"a": "1", "b": "2", "c": "3", "d": "4", "e": "5"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 5,
        map_max_required_keys: Some(1000), // Very high, effectively no limit
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // Should be treated as map since high threshold allows it
    let many_required = &result.schema["properties"]["many_required"];
    assert!(many_required.get("additionalProperties").is_some());
    assert!(many_required.get("properties").is_none());

    println!("✅ Very high max_required_keys threshold works correctly");
}

#[test]
fn test_combination_with_existing_features() {
    // Test interaction with force_field_types and other existing features
    let json_strings = vec![
        r#"{"forced_map": {"a": "1", "b": "2"}, "natural_record": {"x": "val", "y": "val"}}"#
            .to_string(),
    ];

    let mut force_types = std::collections::HashMap::new();
    force_types.insert("forced_map".to_string(), "map".to_string());
    force_types.insert("natural_record".to_string(), "record".to_string());

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(0), // Would block both normally
        force_field_types: force_types,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    // forced_map: should be map due to force override
    let forced = &result.schema["properties"]["forced_map"];
    assert!(forced.get("additionalProperties").is_some());

    // natural_record: should be record due to force override
    let natural = &result.schema["properties"]["natural_record"];
    assert!(natural.get("properties").is_some());

    println!("✅ Combines correctly with existing force_field_types feature");
}

#[test]
fn test_ndjson_with_map_max_required_keys() {
    let ndjson_input = r#"
{"user": {"id": 1, "name": "Alice"}, "tags": {"category": "premium", "source": "web"}}
{"user": {"id": 2, "name": "Bob"}, "tags": {"category": "basic"}}
{"user": {"id": 3, "name": "Charlie"}, "tags": {"category": "premium", "region": "EU"}}
"#;

    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 2,
        map_max_required_keys: Some(1),
        ..Default::default()
    };

    let json_strings = vec![ndjson_input.to_string()];
    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("NDJSON with map_max_required_keys should succeed");

    let schema_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("NDJSON with map_max_rk schema:\n{}", schema_str);

    // user: id and name both required (2 > 1) → Record
    let user = &result.schema["properties"]["user"];
    assert!(user.get("properties").is_some());

    // tags: only category is required (1 ≤ 1) → Map
    let tags = &result.schema["properties"]["tags"];
    assert!(tags.get("additionalProperties").is_some());

    println!("✅ NDJSON works correctly with map_max_required_keys");
}
