use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_decimal_variations() {
    let schema = Schema::from_iter(vec![
        Field::new("decimal_full".into(), DataType::Decimal(10, 2)),
        Field::new("decimal_no_scale".into(), DataType::Decimal(5, 0)),
        Field::new("decimal_no_precision".into(), DataType::Decimal(38, 3)),
        Field::new("decimal_none".into(), DataType::Decimal(38, 0)),
        Field::new("decimal_high_precision".into(), DataType::Decimal(18, 6)),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("decimal_variations", json_schema);
}
