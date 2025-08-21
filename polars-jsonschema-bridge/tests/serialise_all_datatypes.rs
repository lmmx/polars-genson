use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn snapshot_all_supported_types() {
    // Create empty categories using the correct type
    let categories = Categories::new(
        PlSmallStr::from_static("test_categories"),
        PlSmallStr::from_static("test_namespace"),
        CategoricalPhysical::U32, // or whatever the default physical type is
    );

    // Create FrozenCategories for enum data
    let frozen_categories = categories.freeze();

    // Construct a schema with one column of each type we support
    let schema = Schema::from_iter(vec![
        Field::new("bool_col".into(), DataType::Boolean),
        Field::new("int_col".into(), DataType::Int64),
        Field::new("uint_col".into(), DataType::UInt32),
        Field::new("float_col".into(), DataType::Float64),
        Field::new("string_col".into(), DataType::String),
        Field::new("date_col".into(), DataType::Date),
        Field::new(
            "datetime_col".into(),
            DataType::Datetime(TimeUnit::Milliseconds, None),
        ),
        Field::new(
            "datetime_tz_col".into(),
            DataType::Datetime(TimeUnit::Milliseconds, Some(TimeZone::UTC)),
        ),
        Field::new("time_col".into(), DataType::Time),
        Field::new(
            "duration_col".into(),
            DataType::Duration(TimeUnit::Milliseconds),
        ),
        Field::new("list_col".into(), DataType::List(Box::new(DataType::Int64))),
        Field::new(
            "array_col".into(),
            DataType::Array(Box::new(DataType::Float32), 3),
        ),
        Field::new(
            "struct_col".into(),
            DataType::Struct(vec![
                Field::new("inner_str".into(), DataType::String),
                Field::new("inner_int".into(), DataType::Int32),
            ]),
        ),
        Field::new("binary_col".into(), DataType::Binary),
        Field::new("decimal_col".into(), DataType::Decimal(Some(10), Some(2))),
        Field::new("null_col".into(), DataType::Null),
        Field::new(
            "categorical_col".into(),
            DataType::Categorical(categories.clone(), Arc::new(CategoricalMapping::new(1000))),
        ),
        Field::new(
            "enum_col".into(),
            DataType::Enum(
                frozen_categories.clone(),
                Arc::new(CategoricalMapping::new(1000)),
            ),
        ),
    ]);

    let options = JsonSchemaOptions::new()
        .with_title(Some("All Types Example"))
        .with_additional_properties(false);

    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("all_types_schema", json_schema);
}
