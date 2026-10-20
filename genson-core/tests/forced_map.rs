use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// The value schema of `m` when `m` is forced to be a map.
fn forced_map_values(rows: &[&str]) -> Value {
    let config = SchemaInferenceConfig {
        force_field_types: [("m".to_string(), "map".to_string())].into(),
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(rows, config).unwrap().schema;
    let m = &schema["properties"]["m"];
    assert!(m.get("properties").is_none(), "m should be a map: {m}");
    m["additionalProperties"].clone()
}

/// A forced map keeps its values' type rather than turning them into strings.
#[test]
fn test_forced_map_keeps_integer_values() {
    let values = forced_map_values(&[r#"{"m": {"a": 1, "b": 2}}"#, r#"{"m": {"c": 3}}"#]);
    assert_eq!(values["type"], json!("integer"));
}

#[test]
fn test_forced_map_keeps_string_values() {
    let values = forced_map_values(&[r#"{"m": {"en": "hi", "fr": "salut"}}"#]);
    assert_eq!(values["type"], json!("string"));
}

#[test]
fn test_forced_map_of_records_keeps_the_record() {
    let values = forced_map_values(&[r#"{"m": {"a": {"n": 1}, "b": {"n": 2}}}"#]);
    assert_eq!(values["type"], json!("object"));
    assert_eq!(values["properties"]["n"]["type"], json!("integer"));
}

/// Records with different fields unify into one record type with every field.
#[test]
fn test_forced_map_unifies_differing_records() {
    let values = forced_map_values(&[r#"{"m": {"a": {"n": 1}, "b": {"s": "x"}}}"#]);
    let props = values["properties"].as_object().unwrap();
    assert!(
        props.contains_key("n") && props.contains_key("s"),
        "{values}"
    );
}

/// Values of scalar kinds that clash keep a field each, so no value is lost and none is
/// turned into text.
#[test]
fn test_forced_map_with_mixed_scalar_values_keeps_each_kind() {
    let values = forced_map_values(&[r#"{"m": {"a": 1, "b": "x"}}"#]);
    let fields: Vec<&String> = values["properties"].as_object().unwrap().keys().collect();
    assert_eq!(fields, ["m__integer", "m__string"], "{values}");
}

/// A value that is `null` in some rows makes the values nullable, not text.
#[test]
fn test_forced_map_with_null_values_keeps_their_type() {
    let values = forced_map_values(&[r#"{"m": {"k": 1, "j": null}}"#, r#"{"m": {"q": 2}}"#]);
    assert_eq!(values["type"], json!(["null", "integer"]), "{values}");
}

/// A field holding an empty array in some values and items in others unifies to an array
/// of the items' type (an empty array's schema has no `items`), not a string fallback:
/// Wikidata sitelinks' `badges`.
#[test]
fn test_forced_map_unifies_empty_and_non_empty_arrays() {
    let separate_rows = forced_map_values(&[
        r#"{"m": {"enwiki": {"title": "A", "badges": ["Q17437796"]}}}"#,
        r#"{"m": {"dewiki": {"title": "B", "badges": []}}}"#,
    ]);
    let same_row = forced_map_values(&[
        r#"{"m": {"enwiki": {"title": "A", "badges": ["Q17437796"]}, "dewiki": {"title": "B", "badges": []}}}"#,
    ]);
    for values in [separate_rows, same_row] {
        assert_eq!(values["type"], json!("object"), "{values}");
        let badges = &values["properties"]["badges"];
        assert_eq!(badges["type"], json!("array"), "{values}");
        assert_eq!(badges["items"]["type"], json!("string"), "{values}");
    }
}

/// Values that only ever hold empty arrays in a field still unify, keeping the array.
#[test]
fn test_forced_map_unifies_records_with_only_empty_arrays() {
    let values = forced_map_values(&[
        r#"{"m": {"enwiki": {"title": "A", "badges": []}}}"#,
        r#"{"m": {"dewiki": {"title": "B", "badges": []}, "frwiki": {"title": "C"}}}"#,
    ]);
    assert_eq!(values["type"], json!("object"), "{values}");
    // Nullable, as one value lacks it
    assert_eq!(
        values["properties"]["badges"]["type"],
        json!(["null", "array"]),
        "{values}"
    );
}

/// Above PARALLEL_THRESHOLD (10) strings, each string's schema is built and forced on its
/// own before they are merged; a forced map keeps its values' type there too.
fn forced_map_values_parallel(rows: &[&str]) -> Value {
    assert!(rows.len() >= 10, "needs the parallel path");
    forced_map_values(rows)
}

#[test]
fn test_forced_map_of_records_keeps_the_record_in_parallel() {
    let row = r#"{"m": {"enwiki": {"title": "A", "badges": []}}}"#;
    let values = forced_map_values_parallel(&[row; 12]);
    assert_eq!(values["type"], json!("object"), "{values}");
    assert_eq!(
        values["properties"]["title"]["type"],
        json!("string"),
        "{values}"
    );
}

/// Empty maps in other strings (Wikidata items without sitelinks) leave the values' type.
#[test]
fn test_forced_map_with_empty_rows_keeps_the_record_in_parallel() {
    let mut rows = vec![r#"{"m": {}}"#; 10];
    rows.push(r#"{"m": {"specieswiki": {"title": "A", "badges": []}}}"#);
    rows.push(r#"{"m": {"enwiki": {"title": "B", "badges": ["Q17437796"]}}}"#);
    let values = forced_map_values_parallel(&rows);
    assert_eq!(values["type"], json!("object"), "{values}");
    let badges = &values["properties"]["badges"];
    assert_eq!(badges["items"]["type"], json!("string"), "{values}");
}

#[test]
fn test_forced_map_all_empty_is_null_in_parallel() {
    let values = forced_map_values_parallel(&[r#"{"m": {}}"#; 12]);
    assert_eq!(values, json!({"type": "null"}));
}

#[test]
fn test_forced_map_integer_values_in_parallel() {
    let rows: Vec<String> = (0..12)
        .map(|i| format!(r#"{{"m": {{"k{i}": {i}}}}}"#))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let values = forced_map_values_parallel(&rows);
    assert_eq!(values["type"], json!("integer"), "{values}");
}

/// A field forced to a map that is a scalar in some rows, or all, takes the scalar with its
/// values (normalisation wraps the scalar into the map as a value): before, the union left
/// the map with nothing to type, and its values were typed null though they held strings.
#[test]
fn test_forced_map_beside_a_scalar_takes_the_scalar_with_its_values() {
    for (rows, t) in [
        (&[r#"{"m": {"en": "x"}}"#, r#"{"m": "s"}"#][..], "string"),
        (&[r#"{"m": "s"}"#, r#"{"m": {"en": "x"}}"#][..], "string"),
        (&[r#"{"m": {"a": 1}}"#, r#"{"m": 5}"#][..], "integer"),
        (&[r#"{"m": {}}"#, r#"{"m": "s"}"#][..], "string"),
        (&[r#"{"m": "s"}"#][..], "string"),
    ] {
        let values = forced_map_values(rows);
        assert_eq!(values["type"], json!(t), "{rows:?}: {values}");
    }
    // An integer value beside a string: each keeps a field of its own
    let values = forced_map_values(&[r#"{"m": {"a": 1}}"#, r#"{"m": "s"}"#]);
    let fields: Vec<&String> = values["properties"].as_object().unwrap().keys().collect();
    assert_eq!(fields, ["m__integer", "m__string"], "{values}");
}

/// A forced map whose values' records hold a field of two kinds in some and null in others:
/// the field's types made nullable stay one list (`["null", "boolean", "integer"]`, then
/// promoted), where they were nested in another (`["null", ["boolean", "integer"]]`), which
/// no schema reader takes, so inference failed. Ten rows or more take the parallel path.
#[test]
fn test_forced_map_values_nullable_beside_two_kinds_infer() {
    let mut rows = vec![
        r#"{"m": {"a": [{"x": true}, {"x": 1}]}}"#.to_string(),
        r#"{"m": {"b": [{"x": null}]}}"#.to_string(),
    ];
    rows.extend((0..10).map(|i| format!(r#"{{"m": {{"k{i}": [{{"x": {i}}}]}}}}"#)));
    for n in [2, rows.len()] {
        let config = SchemaInferenceConfig {
            force_field_types: [("m".to_string(), "map".to_string())].into(),
            ..SchemaInferenceConfig::default()
        };
        let schema = infer_json_schema_from_strings(&rows[..n], config)
            .unwrap()
            .schema;
        assert!(!schema.to_string().contains(r#"["null",["#), "{schema}");
    }
}

/// A field forced to be a map that holds a list in its rows: the list is a value of the
/// map (normalisation wraps it under a promoted key), its items kept, and the map itself
/// holds no `items`. Its values were `{"type": "array"}`, so every item was written null.
#[test]
fn test_forced_map_over_a_list_keeps_its_items() {
    let values = forced_map_values(&[r#"{"m": [-7]}"#]);
    assert_eq!(
        values,
        json!({"type": "array", "items": {"type": "integer"}})
    );
    let values = forced_map_values(&[r#"{"m": [[""]]}"#]);
    let strings = json!({"type": "array", "items": {"type": "string"}});
    assert_eq!(values, json!({"type": "array", "items": strings}));
    let config = SchemaInferenceConfig {
        force_field_types: [("m".to_string(), "map".to_string())].into(),
        ..SchemaInferenceConfig::default()
    };
    let schema = infer_json_schema_from_strings(&[r#"{"m": [{"d": 1}]}"#], config)
        .unwrap()
        .schema;
    let m = &schema["properties"]["m"];
    assert!(m.get("items").is_none(), "{m}");
    assert_eq!(
        m["additionalProperties"]["items"]["properties"]["d"]["type"],
        "integer"
    );
}

/// The schema of a field with a forced map in a record in one row and in a list's
/// records in another: one list, its records' maps unified as one forced map's values
/// are. The two maps were left apart where their values differed, so the field stayed a
/// union, and normalisation wrote the list's maps as the record's (here null).
#[test]
fn test_forced_maps_in_a_record_and_a_list_unify() {
    let config = SchemaInferenceConfig {
        force_field_types: [("m".to_string(), "map".to_string())].into(),
        ..SchemaInferenceConfig::default()
    };
    let rows = [r#"{"d": {"m": 1}}"#, r#"{"d": [{"m": "s"}]}"#];
    let schema = infer_json_schema_from_strings(&rows, config)
        .unwrap()
        .schema;
    let d = &schema["properties"]["d"];
    assert!(d.get("anyOf").is_none(), "{d}");
    assert_eq!(d["type"], "array", "{d}");
    let values = &d["items"]["properties"]["m"]["additionalProperties"];
    let mut fields: Vec<&String> = values["properties"].as_object().unwrap().keys().collect();
    fields.sort();
    assert_eq!(fields, ["m__integer", "m__string"], "{values}");
}

/// A forced map's values that are lists of kinds that clash within them unify once
/// rewritten (each kind a field), rather than falling back to text.
#[test]
fn test_forced_map_values_lists_of_clashing_kinds_unify() {
    let values = forced_map_values(&[r#"{"m": {"a": ["s"], "b": [-7, "s"]}}"#]);
    assert_eq!(values["type"], "array", "{values}");
    assert_eq!(values["items"]["type"], "object", "{values}");
}

/// The schema with `a` and `c` forced to be maps.
fn forced_a_and_c(rows: &[&str]) -> Value {
    let config = SchemaInferenceConfig {
        force_field_types: [("a", "map"), ("c", "map")]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .into(),
        ..SchemaInferenceConfig::default()
    };
    infer_json_schema_from_strings(rows, config).unwrap().schema
}

/// A field forced within a forced map's values is forced before the values are unified:
/// `[]` and `{"b": 1}` are the values of two maps (a list's, and an integer), not a list of
/// records whose `b` is lost.
#[test]
fn test_fields_forced_within_a_forced_maps_values_first() {
    let schema = forced_a_and_c(&[r#"{"a": {"b": {"c": []}, "d": {"c": {"b": 1}}}}"#]);
    let c = &schema["properties"]["a"]["additionalProperties"]["properties"]["c"];
    let values = &c["additionalProperties"];
    assert_eq!(
        values,
        &json!({"type": "array", "items": {"type": "integer"}}),
        "{c}"
    );
}

/// A forced map's keys are not fields, though they share a forced field's name: `a` in
/// `{"a": {"a": 0}}` is a key, its value an integer beside the other row's.
#[test]
fn test_a_forced_maps_keys_are_not_forced() {
    let schema = forced_a_and_c(&[r#"{"a": -7}"#, r#"{"a": {"a": 0}}"#]);
    assert_eq!(
        schema["properties"]["a"]["additionalProperties"],
        json!({"type": "integer"})
    );
}
