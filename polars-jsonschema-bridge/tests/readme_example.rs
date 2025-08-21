use insta::assert_yaml_snapshot;

#[test]
fn readme_polars_schema_to_json_schema() {
    use polars::prelude::*;
    use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

    let mut schema = Schema::default();
    schema.with_column("name".into(), DataType::String);
    schema.with_column("age".into(), DataType::Int64);
    schema.with_column("scores".into(), DataType::List(Box::new(DataType::Float64)));

    // Default schema
    let json_schema = polars_schema_to_json_schema(&schema, &JsonSchemaOptions::new()).unwrap();
    assert_yaml_snapshot!("readme_default_schema", json_schema);

    // Custom options
    let options = JsonSchemaOptions::new()
        .with_title(Some("User Schema"))
        .with_optional_fields(vec!["scores"]);
    let json_schema_custom = polars_schema_to_json_schema(&schema, &options).unwrap();
    assert_yaml_snapshot!("readme_custom_schema", json_schema_custom);
}

#[test]
fn readme_output_check() -> eyre::Result<()> {
    use polars::prelude::*;
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
