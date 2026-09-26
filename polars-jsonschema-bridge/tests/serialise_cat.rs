use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_categorical_physical_types() {
    let categories_u8 = Categories::new(
        PlSmallStr::from_static("test_u8"),
        PlSmallStr::from_static("test_namespace"),
        CategoricalPhysical::U8,
    );

    let categories_u16 = Categories::new(
        PlSmallStr::from_static("test_u16"),
        PlSmallStr::from_static("test_namespace"),
        CategoricalPhysical::U16,
    );

    let categories_u32 = Categories::new(
        PlSmallStr::from_static("test_u32"),
        PlSmallStr::from_static("test_namespace"),
        CategoricalPhysical::U32,
    );

    let schema = Schema::from_iter(vec![
        Field::new(
            "cat_u8".into(),
            DataType::Categorical(categories_u8.clone(), categories_u8.mapping()),
        ),
        Field::new(
            "cat_u16".into(),
            DataType::Categorical(categories_u16.clone(), categories_u16.mapping()),
        ),
        Field::new(
            "cat_u32".into(),
            DataType::Categorical(categories_u32.clone(), categories_u32.mapping()),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("categorical_physical_types", json_schema);
}
