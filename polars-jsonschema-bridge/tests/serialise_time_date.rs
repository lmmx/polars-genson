use insta::assert_yaml_snapshot;
use polars::prelude::*;
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};

#[test]
fn test_datetime_time_units() {
    let schema = Schema::from_iter(vec![
        Field::new(
            "datetime_ns".into(),
            DataType::Datetime(TimeUnit::Nanoseconds, None),
        ),
        Field::new(
            "datetime_us".into(),
            DataType::Datetime(TimeUnit::Microseconds, None),
        ),
        Field::new(
            "datetime_ms".into(),
            DataType::Datetime(TimeUnit::Milliseconds, None),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("datetime_time_units", json_schema);
}

#[test]
fn test_duration_time_units() {
    let schema = Schema::from_iter(vec![
        Field::new(
            "duration_ns".into(),
            DataType::Duration(TimeUnit::Nanoseconds),
        ),
        Field::new(
            "duration_us".into(),
            DataType::Duration(TimeUnit::Microseconds),
        ),
        Field::new(
            "duration_ms".into(),
            DataType::Duration(TimeUnit::Milliseconds),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("duration_time_units", json_schema);
}

#[test]
fn test_timezone_variations() {
    let schema = Schema::from_iter(vec![
        Field::new(
            "utc".into(),
            DataType::Datetime(TimeUnit::Milliseconds, Some(TimeZone::UTC)),
        ),
        Field::new(
            "ny_tz".into(),
            DataType::Datetime(
                TimeUnit::Milliseconds,
                Some(unsafe { TimeZone::from_static("America/New_York") }),
            ),
        ),
        Field::new(
            "london_tz".into(),
            DataType::Datetime(
                TimeUnit::Microseconds,
                Some(unsafe { TimeZone::from_static("Europe/London") }),
            ),
        ),
    ]);

    let options = JsonSchemaOptions::new();
    let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

    assert_yaml_snapshot!("timezone_variations", json_schema);
}
