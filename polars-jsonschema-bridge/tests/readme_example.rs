use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn readme_polars_schema_to_json_schema() {
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
