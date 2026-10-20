#![cfg(feature = "avro")]
//! A field whose rows hold scalars of kinds that do not widen to one (an integer and a
//! string, a boolean and a number, ...) keeps each kind in a field of its own, as a scalar
//! beside a record does (`a__integer`, `a__string`): no value is dropped and none changes
//! its type. Integers and numbers alone still widen to a number.

use genson_core::normalise::{normalise_values, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

fn config() -> SchemaInferenceConfig {
    SchemaInferenceConfig {
        avro: true,
        ..SchemaInferenceConfig::default()
    }
}

fn schema(rows: &[&str], config: SchemaInferenceConfig) -> Value {
    infer_json_schema_from_strings(rows, config).unwrap().schema
}

fn infer_and_normalise(rows: &[&str], config: SchemaInferenceConfig) -> Vec<Value> {
    let schema = schema(rows, config);
    let values = rows
        .iter()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    normalise_values(values, &schema, &NormaliseConfig::default())
}

/// Every scalar in a JSON value, a number as a float (an integer widened to a number is
/// the same value).
fn scalars(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(o) => o.values().for_each(|v| scalars(v, out)),
        Value::Array(a) => a.iter().for_each(|v| scalars(v, out)),
        Value::Null => {}
        Value::Number(n) => out.push(format!("{:?}", n.as_f64())),
        v => out.push(v.to_string()),
    }
}

/// Each row's scalars are all in its normalised row, with their types.
fn assert_kept(rows: &[&str], config: SchemaInferenceConfig) {
    let out = infer_and_normalise(rows, config);
    for (row, normalised) in rows.iter().zip(&out) {
        let (mut want, mut have) = (Vec::new(), Vec::new());
        scalars(&serde_json::from_str(row).unwrap(), &mut want);
        scalars(normalised, &mut have);
        for scalar in want {
            let at = have.iter().position(|h| *h == scalar);
            let at = at.unwrap_or_else(|| panic!("{scalar} lost from {row}: {normalised}"));
            have.swap_remove(at);
        }
    }
}

/// All the orders of a few rows.
fn permutations<'a>(rows: &[&'a str]) -> Vec<Vec<&'a str>> {
    if rows.len() <= 1 {
        return vec![rows.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..rows.len() {
        let mut rest = rows.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first);
            all.push(tail);
        }
    }
    all
}

/// The same schema whatever order the rows come in, and from the `Value` rewrite (used when
/// debugging) as from the DAG rewrite.
fn assert_order_free(rows: &[&str]) {
    let expected = schema(rows, config());
    for order in permutations(rows) {
        assert_eq!(schema(&order, config()), expected, "rows {order:?}");
    }
    let debugged = SchemaInferenceConfig {
        debug: true,
        ..config()
    };
    assert_eq!(schema(rows, debugged), expected, "the Value rewrite");
}

#[test]
fn test_integer_and_string_each_kept_in_its_own_field() {
    let rows = [r#"{"a": 1}"#, r#"{"a": "x"}"#];
    let out = infer_and_normalise(&rows, config());
    assert_eq!(out[0], json!({"a": {"a__integer": 1, "a__string": null}}));
    assert_eq!(out[1], json!({"a": {"a__integer": null, "a__string": "x"}}));
}

#[test]
fn test_three_kinds_with_null_and_missing_rows() {
    let rows = [
        r#"{"a": 1}"#,
        r#"{"a": "x"}"#,
        r#"{"a": true}"#,
        r#"{"a": null}"#,
        r#"{}"#,
    ];
    let out = infer_and_normalise(&rows, config());
    assert_eq!(out[2]["a"]["a__boolean"], json!(true));
    assert_eq!(out[0]["a"]["a__integer"], json!(1));
    assert_kept(&rows, config());
    assert_order_free(&rows[..4]);
}

#[test]
fn test_boolean_and_integer_kept() {
    let rows = [r#"{"a": true}"#, r#"{"a": 3}"#];
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_integer_and_number_still_widen_to_a_number() {
    let rows = [r#"{"a": 1}"#, r#"{"a": 1.5}"#];
    let fields = &schema(&rows, config())["fields"];
    assert_eq!(fields[0]["type"], json!("float"));
    assert_kept(&rows, config());
}

#[test]
fn test_integer_number_and_string_each_kind_kept() {
    let rows = [r#"{"a": 1}"#, r#"{"a": 1.5}"#, r#"{"a": "x"}"#];
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_mixed_scalars_in_a_list() {
    let rows = [r#"{"a": [1, "x", true]}"#, r#"{"a": [2]}"#];
    let out = infer_and_normalise(&rows, config());
    assert_eq!(out[0]["a"][1]["__string"], json!("x"));
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_mixed_scalars_beside_records_in_nested_lists() {
    let rows = [r#"{"a": [[1], ["x"], [{"b": null}]]}"#];
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_scalar_beside_an_empty_record() {
    let rows = [r#"{"c": {}}"#, r#"{"c": 3}"#];
    let out = infer_and_normalise(&rows, config());
    assert_eq!(out[1], json!({"c": {"c__integer": 3}}));
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_scalar_beside_a_list_of_empty_records() {
    let rows = [r#"{"a": -7}"#, r#"{"a": [{}]}"#];
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_scalar_beside_a_list_of_nulls_joins_the_list() {
    let rows = [r#"{"a": [null]}"#, r#"{"a": 3}"#];
    let out = infer_and_normalise(&rows, config());
    assert_eq!(out[1], json!({"a": [3]}));
    assert_eq!(out[0], json!({"a": [null]}));
    assert_order_free(&rows);
}

#[test]
fn test_record_beside_a_list_of_nulls_joins_the_list() {
    let rows = [r#"{"d": [null]}"#, r#"{"d": {"b": [0]}}"#];
    assert_kept(&rows, config());
    assert_order_free(&rows);
}

#[test]
fn test_mixed_scalars_under_map_options() {
    let maps = SchemaInferenceConfig {
        map_threshold: 0,
        unify_maps: true,
        ..config()
    };
    for rows in [
        &[r#"{"c": ""}"#, r#"{"c": true}"#][..],
        &[r#"{"b": "s"}"#, r#"{"b": 0}"#],
        &[r#"{"a": {"d": [-7, "s"]}}"#],
    ] {
        assert_kept(rows, maps.clone());
    }
}

/// With `wrap_scalars` off, nothing is promoted: the union is left as it was.
#[test]
fn test_union_untouched_without_wrap_scalars() {
    let rows = [r#"{"a": 1}"#, r#"{"a": "x"}"#];
    let off = SchemaInferenceConfig {
        wrap_scalars: false,
        ..config()
    };
    let fields = &schema(&rows, off)["fields"];
    assert_eq!(fields[0]["type"], json!(["int", "string"]));
}
