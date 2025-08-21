use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_array_sizes() {
    let schema = Schema::from_iter(vec![
        Field::new(
            "small_array".into(),
            DataType::Array(Box::new(DataType::Int32), 2),
        ),
        Field::new(
            "medium_array".into(),
            DataType::Array(Box::new(DataType::Float64), 5),
        ),
        Field::new(
            "large_array".into(),
            DataType::Array(Box::new(DataType::String), 10),
        ),
        Field::new(
            "string_array".into(),
            DataType::Array(Box::new(DataType::String), 3),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("array_sizes", json_schema);
}

#[test]
fn test_list_variations() {
    let schema = Schema::from_iter(vec![
        Field::new("list_int".into(), DataType::List(Box::new(DataType::Int64))),
        Field::new(
            "list_float".into(),
            DataType::List(Box::new(DataType::Float64)),
        ),
        Field::new(
            "list_string".into(),
            DataType::List(Box::new(DataType::String)),
        ),
        Field::new(
            "list_bool".into(),
            DataType::List(Box::new(DataType::Boolean)),
        ),
        Field::new("list_date".into(), DataType::List(Box::new(DataType::Date))),
        // Nested lists
        Field::new(
            "list_of_lists".into(),
            DataType::List(Box::new(DataType::List(Box::new(DataType::Int32)))),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("list_variations", json_schema);
}
