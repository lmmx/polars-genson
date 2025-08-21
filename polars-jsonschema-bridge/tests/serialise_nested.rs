use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_nested_structures() {
    let schema = Schema::from_iter(vec![
        // List of structs
        Field::new(
            "list_of_structs".into(),
            DataType::List(Box::new(DataType::Struct(vec![
                Field::new("name".into(), DataType::String),
                Field::new("age".into(), DataType::Int32),
            ]))),
        ),
        // Struct with list
        Field::new(
            "struct_with_list".into(),
            DataType::Struct(vec![
                Field::new("tags".into(), DataType::List(Box::new(DataType::String))),
                Field::new("id".into(), DataType::Int64),
            ]),
        ),
        // Array of structs
        Field::new(
            "array_of_structs".into(),
            DataType::Array(
                Box::new(DataType::Struct(vec![
                    Field::new("x".into(), DataType::Float64),
                    Field::new("y".into(), DataType::Float64),
                ])),
                4,
            ),
        ),
        // Deeply nested
        Field::new(
            "deep_nest".into(),
            DataType::Struct(vec![Field::new(
                "level1".into(),
                DataType::Struct(vec![Field::new(
                    "level2".into(),
                    DataType::List(Box::new(DataType::String)),
                )]),
            )]),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("nested_structures", json_schema);
}
