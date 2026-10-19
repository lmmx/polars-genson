//! A map whose values are typed null (only ever empty, or holding nulls alone) unified with
//! one whose values have a type: `null ⊔ T` is nullable `T`, and `null ⊔ null` is null.
#![cfg(feature = "avro")]

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// `c` (three keys) is a map candidate and its records (two keys) are not; `m` is forced to
/// be a map. Returns the schema of `m` in `c`'s values.
fn m_in_c(rows: &[&str], avro: bool) -> Value {
    let config = SchemaInferenceConfig {
        force_field_types: [("m".to_string(), "map".to_string())].into(),
        map_threshold: 2,
        unify_maps: true,
        avro,
        ..SchemaInferenceConfig::default()
    };
    infer_json_schema_from_strings(rows, config).unwrap().schema
}

fn m_schema(rows: &[&str]) -> Value {
    let schema = m_in_c(rows, false);
    let c = &schema["properties"]["c"];
    assert!(
        c.get("additionalProperties").is_some(),
        "c should be a map: {c}"
    );
    c["additionalProperties"]["properties"]["m"].clone()
}

/// The three `c` entries, `m` in each as given.
fn row(m: [&str; 3]) -> String {
    format!(
        r#"{{"c": {{"P1": {{"id": "Q1", "m": {}}}, "P2": {{"id": "Q2", "m": {}}}, "P3": {{"id": "Q3", "m": {}}}}}}}"#,
        m[0], m[1], m[2]
    )
}

#[test]
fn test_null_values_beside_typed_values_are_nullable() {
    for m in [
        // only ever empty beside strings
        [r#"{}"#, r#"{"en": "x"}"#, r#"{"fr": "y"}"#],
        // holding nulls alone beside strings: the nulls must stay possible
        [r#"{"a": null}"#, r#"{"en": "x"}"#, r#"{"fr": "y"}"#],
        // and in another order
        [r#"{"en": "x"}"#, r#"{"a": null}"#, r#"{}"#],
    ] {
        let m_schema = m_schema(&[&row(m)]);
        assert_eq!(m_schema["type"], json!("object"), "{m:?}: {m_schema}");
        assert_eq!(
            m_schema["additionalProperties"],
            json!({"type": ["null", "string"]}),
            "{m:?}: {m_schema}"
        );
    }
}

#[test]
fn test_null_values_beside_null_values_stay_null() {
    for m in [
        [r#"{}"#, r#"{}"#, r#"{}"#],
        [r#"{"a": null}"#, r#"{}"#, r#"{"b": null}"#],
    ] {
        let m_schema = m_schema(&[&row(m)]);
        assert_eq!(
            m_schema["additionalProperties"],
            json!({"type": "null"}),
            "{m_schema}"
        );
    }
}

/// A map that is null in some rows stays nullable when it takes the other's values.
#[test]
fn test_a_nullable_map_with_null_values_stays_nullable() {
    let rows = [
        row([r#"{}"#, r#"{"en": "x"}"#, r#"{"fr": "y"}"#]),
        row([r#"null"#, r#"{"en": "z"}"#, r#"{"fr": "w"}"#]),
    ];
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let m_schema = m_schema(&rows);
    assert_eq!(m_schema["type"], json!(["null", "object"]), "{m_schema}");
    assert_eq!(
        m_schema["additionalProperties"],
        json!({"type": ["null", "string"]}),
        "{m_schema}"
    );
}

/// Inside a record inside the map's values, the same.
#[test]
fn test_null_values_in_a_nested_record() {
    let rows = [
        r#"{"c": {"P1": {"id": "Q1", "r": {"m": {}}}, "P2": {"id": "Q2", "r": {"m": {"en": "x"}}}, "P3": {"id": "Q3", "r": {"m": {"a": null}}}}}"#,
    ];
    let schema = m_in_c(&rows, false);
    let m =
        &schema["properties"]["c"]["additionalProperties"]["properties"]["r"]["properties"]["m"];
    assert_eq!(
        m["additionalProperties"],
        json!({"type": ["null", "string"]}),
        "{schema}"
    );
}

/// Normalised against the Avro schema, every value is kept, the nulls included, and the
/// rows decode to Arrow.
#[cfg(feature = "parquet")]
#[test]
fn test_null_values_beside_typed_values_normalise_and_decode() {
    use genson_core::normalise::{normalise_values, MapEncoding, NormaliseConfig};
    use genson_core::parquet::{avro_record_fields, values_to_struct_array};
    let rows = [row([r#"{"a": null}"#, r#"{"en": "x"}"#, r#"{}"#])];
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let schema = m_in_c(&rows, true);
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    let values: Vec<Value> = rows
        .iter()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    let normalised = normalise_values(values, &schema, &cfg);
    let c = &normalised[0]["c"];
    let m = |k: &str| {
        c.as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == k)
            .unwrap()["value"]["m"]
            .clone()
    };
    assert_eq!(m("P1"), json!([{"key": "a", "value": null}]), "{c}");
    assert_eq!(m("P2"), json!([{"key": "en", "value": "x"}]), "{c}");
    assert_eq!(m("P3"), json!([]), "{c}");
    let fields = avro_record_fields(&schema, false).unwrap();
    values_to_struct_array(&normalised, &fields).unwrap();
}
