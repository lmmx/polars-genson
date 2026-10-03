#![cfg(all(feature = "avro", feature = "parquet"))]

use genson_core::normalise::{normalise_values, MapEncoding, NormaliseConfig};
use genson_core::parquet::{avro_record_fields, values_to_struct_array, write_struct_column};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// Rows whose `aliases` is `{}` in every row (Wikidata items without aliases).
const ALL_EMPTY: [&str; 2] = [r#"{"aliases": {}}"#, r#"{"aliases": {}}"#];

fn aliases_schema(config: SchemaInferenceConfig) -> Value {
    let schema = infer_json_schema_from_strings(&ALL_EMPTY, config).unwrap().schema;
    schema["properties"]["aliases"].clone()
}

/// An object empty in every row, where every object is a map candidate
/// (`map_threshold: 0`), is a map with null values, not a record with no fields
/// (which Parquet cannot write).
#[test]
fn test_always_empty_object_is_null_map_at_threshold_zero() {
    let aliases = aliases_schema(SchemaInferenceConfig {
        map_threshold: 0,
        ..SchemaInferenceConfig::default()
    });
    assert_eq!(aliases["additionalProperties"], json!({"type": "null"}), "{aliases}");
}

/// The same when the object is forced to be a map (its values were a string fallback).
#[test]
fn test_always_empty_forced_map_has_null_values() {
    let aliases = aliases_schema(SchemaInferenceConfig {
        force_field_types: [("aliases".to_string(), "map".to_string())].into(),
        ..SchemaInferenceConfig::default()
    });
    assert_eq!(aliases["additionalProperties"], json!({"type": "null"}), "{aliases}");
}

/// Above the threshold (the default 20), an always-empty object is left as it was.
#[test]
fn test_always_empty_object_unchanged_below_threshold() {
    let aliases = aliases_schema(SchemaInferenceConfig::default());
    assert_eq!(aliases, json!({"type": "object"}));
}

/// The typed output of the always-empty map writes to Parquet: `List<Struct{key, value:
/// Null}>`, one empty list per row.
#[test]
fn test_always_empty_map_writes_to_parquet() {
    let config = SchemaInferenceConfig {
        map_threshold: 0,
        avro: true,
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(&ALL_EMPTY, config).unwrap().schema;
    let rows: Vec<Value> = ALL_EMPTY.iter().map(|r| serde_json::from_str(r).unwrap()).collect();
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    let normalised = normalise_values(rows, &schema, &cfg);
    let fields = avro_record_fields(&schema).unwrap();
    let array = values_to_struct_array(&normalised, &fields).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.parquet");
    write_struct_column(path.to_str().unwrap(), "aliases", vec![array], &fields, None).unwrap();

    let file = std::fs::File::open(&path).unwrap();
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let rows: usize = reader.map(|b| b.unwrap().num_rows()).sum();
    assert_eq!(rows, 2);
}
