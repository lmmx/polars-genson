// Block 1 - SKIPPED: Cargo.toml dependencies (not testable Rust code)

/// Block 2 - JSON Schema to Polars conversion with complex nested structure
#[test]
fn readme_json_schema_to_polars() -> eyre::Result<()> {
    use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
    use serde_json::json;

    let json_schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"},
            "scores": {
                "type": "array",
                "items": {"type": "number"}
            },
            "profile": {
                "type": "object",
                "properties": {
                    "active": {"type": "boolean"}
                }
            }
        }
    });

    let fields = schema_to_polars_fields(&json_schema, SchemaFormat::JsonSchema, false)?;

    // Convert to a more snapshot-friendly format
    let fields_vec: Vec<(String, String)> = fields.into_iter().collect();

    insta::assert_yaml_snapshot!("readme_json_to_polars", fields_vec);
    Ok(())
}

/// Block 3 - Basic Polars schema to JSON Schema conversion
#[test]
fn readme_polars_to_json_basic() -> eyre::Result<()> {
    use polars::prelude::*;
    use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

    let mut schema = Schema::default();
    schema.with_column("name".into(), DataType::String);
    schema.with_column("age".into(), DataType::Int64);
    schema.with_column("scores".into(), DataType::List(Box::new(DataType::Float64)));

    // Use default options
    let json_schema = polars_schema_to_json_schema(&schema, &JsonSchemaOptions::new())?;

    insta::assert_yaml_snapshot!("readme_basic_schema", json_schema);

    Ok(())
}

/// Block 4 - Polars schema to JSON Schema with custom options
#[test]
fn readme_polars_to_json_custom() -> eyre::Result<()> {
    use polars::prelude::*;
    use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

    let mut schema = Schema::default();
    schema.with_column("name".into(), DataType::String);
    schema.with_column("age".into(), DataType::Int64);
    schema.with_column("scores".into(), DataType::List(Box::new(DataType::Float64)));

    // Or customize the output with options
    let options = JsonSchemaOptions::new()
        .with_title(Some("User Schema"))
        .with_description(Some("A simple user record"))
        .with_optional_fields(vec!["email"])
        .with_additional_properties(true);

    let json_schema_custom = polars_schema_to_json_schema(&schema, &options)?;
    insta::assert_yaml_snapshot!("readme_custom_schema", json_schema_custom);

    Ok(())
}

/// Block 5 - Individual type conversions between JSON Schema and Polars
#[test]
fn readme_individual_conversions() -> eyre::Result<()> {
    use polars::prelude::DataType;
    use polars_jsonschema_bridge::{
        json_type_to_polars_type, polars_dtype_to_json_schema, JsonSchemaOptions,
    };
    use serde_json::json;

    // JSON Schema type → Polars type string
    let polars_type = json_type_to_polars_type(&json!({"type": "string"}))?;
    assert_eq!(polars_type, "String");

    // Polars DataType → JSON Schema
    let json_schema = polars_dtype_to_json_schema(
        &DataType::List(Box::new(DataType::Int64)),
        &JsonSchemaOptions::default(),
    )?;
    assert_eq!(
        json_schema,
        json!({
            "type": "array",
            "items": {"type": "integer"}
        })
    );

    Ok(())
}

/// Block 6 - Error handling for unsupported type conversions
#[test]
fn readme_error_handling() -> eyre::Result<()> {
    use polars_jsonschema_bridge::json_type_to_polars_type;
    use serde_json::json;

    // Unsupported types return errors
    let result = json_type_to_polars_type(&json!({"type": "unsupported"}));
    assert!(result.is_err());

    // Test a few more error cases for good measure
    let result2 = json_type_to_polars_type(&json!({"type": "not_a_real_type"}));
    assert!(result2.is_err());

    Ok(())
}

/// Block 7 - JsonSchemaOptions builder pattern example
#[test]
fn readme_json_schema_options() -> eyre::Result<()> {
    use polars::prelude::*;
    use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

    let mut schema = Schema::default();
    schema.with_column("name".into(), DataType::String);
    schema.with_column("email".into(), DataType::String);
    schema.with_column("age".into(), DataType::Int64);

    let options = JsonSchemaOptions::new()
        .with_title(Some("Example"))
        .with_optional_fields(vec!["email"])
        .with_additional_properties(false);

    let json_schema = polars_schema_to_json_schema(&schema, &options)?;
    insta::assert_yaml_snapshot!("readme_options_example", json_schema);

    Ok(())
}

/// Block 8 - Debug mode functionality test
#[test]
fn readme_debug_mode() -> eyre::Result<()> {
    use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
    use serde_json::json;

    let json_schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "active": {"type": "boolean"}
        }
    });

    // Test that debug mode works (debug = true)
    let fields_debug = schema_to_polars_fields(&json_schema, SchemaFormat::JsonSchema, true)?;

    // Test that non-debug mode works (debug = false)
    let fields_normal = schema_to_polars_fields(&json_schema, SchemaFormat::JsonSchema, false)?;

    // Both should produce the same result
    assert_eq!(fields_debug, fields_normal);

    // Ensure we got the expected fields
    let expected_fields = vec![
        ("name".to_string(), "String".to_string()),
        ("active".to_string(), "Boolean".to_string()),
    ];

    assert_eq!(fields_normal, expected_fields);

    Ok(())
}
