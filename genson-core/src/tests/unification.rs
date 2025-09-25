use super::*;
use serde_json::json;
use crate::{infer_json_schema_from_strings, schema::rewrite_objects};

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
    assert_eq!(colors_values["type"], json!("string"));
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

#[test]
fn test_anyof_unification() {
    let mut anyof_schema = json!({
        "anyOf": [
            {"type": "object", "properties": {"timezone": {"type": "integer"}}},
            {"type": "string"}
        ]
    });

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    rewrite_objects(&mut anyof_schema, Some("datavalue"), &config, false);
    println!("Generated schema: {}", serde_json::to_string_pretty(&anyof_schema).unwrap());

    // Should be unified to a single object, not anyOf
    assert!(anyof_schema.get("anyOf").is_none(), "anyOf should be unified away");
    assert_eq!(anyof_schema["type"], "object");
    assert!(anyof_schema.get("properties").is_some(), "Should have properties after unification");
}

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

#[test]
fn test_anyof_rewrite_objects_root() {
    let mut nested_schema = json!({
        "type": "object",
        "properties": {
            "claims": {
                "type": "object",
                "additionalProperties": {
                    "anyOf": [
                        {"type": "object", "properties": {"timezone": {"type": "integer"}}},
                        {"type": "string"}
                    ]
                }
            }
        }
    });

    let config = SchemaInferenceConfig { unify_maps: true, wrap_scalars: true, ..Default::default() };
    rewrite_objects(&mut nested_schema, None, &config, true);

    println!("{}", nested_schema);

    // Should not have anyOf anymore
    assert!(nested_schema["properties"]["claims"]["additionalProperties"].get("anyOf").is_none());
}

#[test]
fn test_anyof_rewrite_objects_nested() {
    let mut nested_schema = json!({
        "type": "object",
        "properties": {
            "claims": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": {
                        "anyOf": [
                            {"type": "object", "properties": {"timezone": {"type": "integer"}}},
                            {"type": "string"}
                        ]
                    }
                }
            }
        }
    });

    let config = SchemaInferenceConfig { unify_maps: true, wrap_scalars: true, ..Default::default() };
    rewrite_objects(&mut nested_schema, None, &config, true);

    println!("{}", nested_schema);

    // This currently fails - anyOf is still there
    let inner_schema = &nested_schema["properties"]["claims"]["additionalProperties"]["additionalProperties"];
    assert!(inner_schema.get("anyOf").is_none(), "anyOf should be unified away");
    assert_eq!(inner_schema["type"], "object");
}

#[test]
fn test_schema_inference_root_anyof() {
    let test_data = vec![
        json!({"datavalue": "string-value"}).to_string(),
        json!({"datavalue": {"timezone": 0}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config).unwrap();
    println!("Root anyOf schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["datavalue"];
    assert_eq!(datavalue_schema["type"], "object");
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema["properties"].get("datavalue__string").is_some());
}

#[test]
fn test_schema_inference_nested_anyof() {
    let test_data = vec![
        json!({"claims": {"P31": {"datavalue": "string-value"}}}).to_string(),
        json!({"claims": {"P31": {"datavalue": {"timezone": 0}}}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config).unwrap();
    println!("Nested anyOf schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["claims"]["additionalProperties"]["additionalProperties"];
    assert_eq!(datavalue_schema["type"], "object");
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema["properties"].get("datavalue__string").is_some());
}

#[test]
fn test_scalar_promotion_integer_vs_number() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "foo": {"type": "integer"}
            },
            "required": ["foo"]
        }),
        json!({
            "type": "object",
            "properties": {
                "foo": {"type": "number"}
            },
            "required": ["foo"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    assert_eq!(unified["type"], "object");

    let props = unified["properties"].as_object().expect("Should have properties");
    assert!(props.contains_key("foo"), "Should have foo field");

    let foo_schema = &props["foo"];
    assert_eq!(foo_schema["type"], "object");

    let foo_props = foo_schema["properties"].as_object().expect("Foo should have properties");
    assert!(foo_props.contains_key("foo__integer"), "Missing promoted integer field");
    assert!(foo_props.contains_key("foo__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_string_vs_boolean() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "bar": {"type": "string"}
            },
            "required": ["bar"]
        }),
        json!({
            "type": "object",
            "properties": {
                "bar": {"type": "boolean"}
            },
            "required": ["bar"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let bar_props = props["bar"]["properties"].as_object().expect("Bar should have properties");

    assert!(bar_props.contains_key("bar__string"), "Missing promoted string field");
    assert!(bar_props.contains_key("bar__boolean"), "Missing promoted boolean field");
}

#[test]
fn test_scalar_promotion_string_vs_number() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "baz": {"type": "string"}
            },
            "required": ["baz"]
        }),
        json!({
            "type": "object",
            "properties": {
                "baz": {"type": "number"}
            },
            "required": ["baz"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let baz_props = props["baz"]["properties"].as_object().expect("Baz should have properties");

    assert!(baz_props.contains_key("baz__string"), "Missing promoted string field");
    assert!(baz_props.contains_key("baz__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_string_vs_integer() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "qux": {"type": "string"}
            },
            "required": ["qux"]
        }),
        json!({
            "type": "object",
            "properties": {
                "qux": {"type": "integer"}
            },
            "required": ["qux"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let qux_props = props["qux"]["properties"].as_object().expect("Qux should have properties");

    assert!(qux_props.contains_key("qux__string"), "Missing promoted string field");
    assert!(qux_props.contains_key("qux__integer"), "Missing promoted integer field");
}

#[test]
fn test_scalar_promotion_boolean_vs_number() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "xyz": {"type": "boolean"}
            },
            "required": ["xyz"]
        }),
        json!({
            "type": "object",
            "properties": {
                "xyz": {"type": "number"}
            },
            "required": ["xyz"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let xyz_props = props["xyz"]["properties"].as_object().expect("Xyz should have properties");

    assert!(xyz_props.contains_key("xyz__boolean"), "Missing promoted boolean field");
    assert!(xyz_props.contains_key("xyz__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_boolean_vs_integer() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "abc": {"type": "boolean"}
            },
            "required": ["abc"]
        }),
        json!({
            "type": "object",
            "properties": {
                "abc": {"type": "integer"}
            },
            "required": ["abc"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let abc_props = props["abc"]["properties"].as_object().expect("Abc should have properties");

    assert!(abc_props.contains_key("abc__boolean"), "Missing promoted boolean field");
    assert!(abc_props.contains_key("abc__integer"), "Missing promoted integer field");
}

#[test]
fn test_scalar_promotion_three_types() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "string"}
            },
            "required": ["multi"]
        }),
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "number"}
            },
            "required": ["multi"]
        }),
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "boolean"}
            },
            "required": ["multi"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let multi_props = props["multi"]["properties"].as_object().expect("Multi should have properties");

    assert!(multi_props.contains_key("multi__string"), "Missing promoted string field");
    assert!(multi_props.contains_key("multi__number"), "Missing promoted number field");
    assert!(multi_props.contains_key("multi__boolean"), "Missing promoted boolean field");
}

#[test]
fn test_scalar_promotion_all_four_types() {
    let schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "string"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "number"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "integer"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "boolean"}
            },
            "required": ["all"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let result = unify_record_schemas(&schemas, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let all_props = props["all"]["properties"].as_object().expect("All should have properties");

    assert!(all_props.contains_key("all__string"), "Missing promoted string field");
    assert!(all_props.contains_key("all__number"), "Missing promoted number field");
    assert!(all_props.contains_key("all__integer"), "Missing promoted integer field");
    assert!(all_props.contains_key("all__boolean"), "Missing promoted boolean field");
}
