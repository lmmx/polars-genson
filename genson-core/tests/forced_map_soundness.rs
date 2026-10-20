#![cfg(feature = "avro")]
//! Every scalar survives inference and normalisation with fields forced to be maps, read
//! each way inference reads rows: one row at a time (fewer than ten rows), the interned
//! rows forced one distinct row at a time (ten or more), and the merged schema rewritten as
//! `Value`s (when debugging) rather than as a DAG. polars-genson-py's
//! forced_map_soundness_test.py checks the same shapes through both decodes.

use genson_core::normalise::{normalise_values, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::Value;

/// The scalars in `value`, numbers compared as numbers.
fn leaves(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(o) => o.values().for_each(|v| leaves(v, out)),
        Value::Array(a) => a.iter().for_each(|v| leaves(v, out)),
        Value::Null => {}
        Value::Number(n) => out.push(format!("number {}", n.as_f64().unwrap())),
        v => out.push(v.to_string()),
    }
}

/// The scalars of `row` not found in `normalised`.
fn lost(row: &Value, normalised: &Value) -> Vec<String> {
    let (mut have, mut want) = (Vec::new(), Vec::new());
    leaves(normalised, &mut have);
    leaves(row, &mut want);
    want.into_iter()
        .filter(|leaf| match have.iter().position(|h| h == leaf) {
            Some(at) => {
                have.remove(at);
                false
            }
            None => true,
        })
        .collect()
}

fn assert_kept(rows: &[&str]) {
    for (copies, debug) in [(1, false), (10, false), (1, true), (10, true)] {
        let rows: Vec<&str> = rows
            .iter()
            .copied()
            .cycle()
            .take(rows.len() * copies)
            .collect();
        let config = SchemaInferenceConfig {
            force_field_types: [("a", "map"), ("c", "map")]
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .into(),
            avro: true,
            debug,
            ..SchemaInferenceConfig::default()
        };
        let schema = infer_json_schema_from_strings(&rows, config)
            .unwrap_or_else(|e| panic!("{copies} copies, debug {debug}: {rows:?}: {e}"))
            .schema;
        let values: Vec<Value> = rows
            .iter()
            .map(|r| serde_json::from_str(r).unwrap())
            .collect();
        let normalised = normalise_values(values.clone(), &schema, &NormaliseConfig::default());
        for (row, out) in values.iter().zip(&normalised) {
            let missing = lost(row, out);
            assert!(
                missing.is_empty(),
                "{copies} copies, debug {debug}: {row} -> {out} lost {missing:?}"
            );
        }
    }
}

#[test]
fn test_forced_map_over_a_list_keeps_every_value() {
    assert_kept(&[r#"{"a": [-7]}"#]);
    assert_kept(&[r#"{"a": [[""]]}"#]);
    assert_kept(&[r#"{"b": {"a": [{"d": 1}]}}"#]);
    assert_kept(&[r#"{"a": [-7]}"#, r#"{"a": {"x": 1}}"#]);
}

#[test]
fn test_forced_maps_in_a_record_and_a_list_keep_every_value() {
    assert_kept(&[r#"{"d": {"a": 1}}"#, r#"{"d": [{"a": "s"}]}"#]);
    assert_kept(&[r#"{"d": {"a": []}}"#, r#"{"d": [{"a": true}]}"#]);
    assert_kept(&[r#"{"b": [{"c": {"a": -7}}]}"#, r#"{"b": {"c": ["42"]}}"#]);
}

#[test]
fn test_forced_map_values_of_clashing_lists_keep_every_value() {
    assert_kept(&[r#"{"d": {"c": {"a": ["s"], "c": [-7, "s"]}}}"#]);
}

#[test]
fn test_fields_forced_within_values_keep_every_value() {
    assert_kept(&[r#"{"a": {"b": {"c": []}, "d": {"c": {"b": 1}}}}"#]);
    assert_kept(&[r#"{"a": {"c": {"a": [false, 1]}, "d": {"a": ["42"]}}}"#]);
    assert_kept(&[r#"{"c": [{"c": {"c": -7}}]}"#, r#"{"c": {"c": {"c": []}}}"#]);
    assert_kept(&[r#"{"a": -7}"#, r#"{"a": {"a": 0}}"#]);
}

/// A forced field met in more distinct records than are unified pairwise (50): its maps,
/// one of records and one of strings, are still unified as a forced map's values are (the
/// string a field of the record), which the halves' map unification does not do.
#[test]
fn test_many_forced_maps_in_one_field_keep_every_value() {
    let mut members: Vec<String> = (0..55)
        .map(|k| format!(r#""k{k}": {{"a": {{"m": {{"x{k}": {k}}}}}}}"#))
        .collect();
    members.push(r#""k55": {"a": {"m": "s"}}"#.to_string());
    let row = format!(r#"{{"c": {{{}}}}}"#, members.join(", "));
    assert_kept(&[&row]);
}
