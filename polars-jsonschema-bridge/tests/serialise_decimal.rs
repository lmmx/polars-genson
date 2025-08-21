use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_decimal_variations() {
    let schema = Schema::from_iter(vec![
        Field::new("decimal_full".into(), DataType::Decimal(Some(10), Some(2))),
        Field::new("decimal_no_scale".into(), DataType::Decimal(Some(5), None)),
        Field::new(
            "decimal_no_precision".into(),
            DataType::Decimal(None, Some(3)),
        ),
        Field::new("decimal_none".into(), DataType::Decimal(None, None)),
        Field::new(
            "decimal_high_precision".into(),
            DataType::Decimal(Some(18), Some(6)),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("decimal_variations", json_schema);
}
