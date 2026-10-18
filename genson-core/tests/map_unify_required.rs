//! Map unification takes `required` from the records it unifies (#50). A record field
//! that some map values lack is optional, and nullable in Avro, whatever the order in which
//! the values are unified: a record that is itself a unified schema lists every field seen
//! so far in its `properties`, required or not.

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// Three values of the map `m`; the nested record `v` of the second has no `b`, so the
/// first two unify to a record whose `b` is optional before the third is unified with it.
const ROWS: [&str; 3] = [
    r#"{"m": {"k1": {"v": {"a": 1, "b": {"x": 1}}}}}"#,
    r#"{"m": {"k2": {"v": {"a": 2}}}}"#,
    r#"{"m": {"k3": {"v": {"a": 3, "b": {"x": 3}}}}}"#,
];

/// `m` (three keys) is a map; `v` (two) and `b` (one) are records.
fn config() -> SchemaInferenceConfig {
    SchemaInferenceConfig {
        map_threshold: 2,
        unify_maps: true,
        ..Default::default()
    }
}

fn infer(rows: &[&str], config: SchemaInferenceConfig) -> Value {
    let rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
    infer_json_schema_from_strings(&rows, config)
        .unwrap()
        .schema
}

/// The JSON Schema of the map's values' record `v`.
fn record_v(schema: &Value) -> &Value {
    let values = &schema["properties"]["m"]["additionalProperties"];
    assert!(values.is_object(), "m should be a map: {schema:#}");
    &values["properties"]["v"]
}

#[test]
fn test_field_missing_from_a_map_value_is_not_required() {
    let schema = infer(&ROWS, config());
    let v = record_v(&schema);
    assert_eq!(v["required"], json!(["a"]), "{v:#}");
    assert_eq!(v["properties"]["a"]["type"], json!("integer"), "{v:#}");
    let b_type = &v["properties"]["b"]["type"];
    assert!(
        b_type
            .as_array()
            .is_some_and(|t| t.contains(&json!("null"))),
        "b should be nullable: {v:#}"
    );
}

#[test]
fn test_required_does_not_depend_on_the_order_of_the_values() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let rows: Vec<&str> = order.iter().map(|&i| ROWS[i]).collect();
        let schema = infer(&rows, config());
        assert_eq!(
            record_v(&schema)["required"],
            json!(["a"]),
            "order {order:?}"
        );
    }
}

#[test]
fn test_field_required_by_every_value_stays_required() {
    // `b` a string, so that `v` (an integer and a string) stays a record
    let rows = [
        r#"{"m": {"k1": {"v": {"a": 1, "b": "x"}}}}"#,
        r#"{"m": {"k2": {"v": {"a": 2, "b": "y"}}}}"#,
        r#"{"m": {"k3": {"v": {"a": 3, "b": "z"}}}}"#,
    ];
    let schema = infer(&rows, config());
    let v = record_v(&schema);
    assert_eq!(v["required"], json!(["a", "b"]), "{v:#}");
    assert_eq!(v["properties"]["b"]["type"], json!("string"), "{v:#}");
}

#[cfg(feature = "avro")]
mod avro {
    use super::*;
    use genson_core::normalise::{normalise_values, NormaliseConfig};

    /// The Avro field `name` of the map's values' record `v`.
    fn avro_field<'a>(schema: &'a Value, name: &str) -> &'a Value {
        let values = &schema["fields"][0]["type"]["values"];
        let v = values["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "v")
            .unwrap();
        let v_record = match &v["type"] {
            Value::Array(branches) => branches.iter().find(|b| b != &&json!("null")).unwrap(),
            record => record,
        };
        v_record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .unwrap()
    }

    #[test]
    fn test_field_missing_from_a_map_value_is_nullable_in_avro() {
        let schema = infer(
            &ROWS,
            SchemaInferenceConfig {
                avro: true,
                ..config()
            },
        );
        let b = avro_field(&schema, "b");
        let nullable = b["type"]
            .as_array()
            .is_some_and(|t| t.contains(&json!("null")));
        assert!(nullable, "b should be nullable: {schema:#}");
    }

    /// A record field missing from the input normalises to null, not to a record whose
    /// fields are all null.
    #[test]
    fn test_missing_record_field_normalises_to_null() {
        let schema = infer(
            &ROWS,
            SchemaInferenceConfig {
                avro: true,
                ..config()
            },
        );
        let rows: Vec<Value> = ROWS
            .iter()
            .map(|r| serde_json::from_str(r).unwrap())
            .collect();
        let out = normalise_values(rows, &schema, &NormaliseConfig::default());
        assert_eq!(
            out[1]["m"]["k2"]["v"],
            json!({"a": 2, "b": null}),
            "{out:#?}"
        );
        assert_eq!(
            out[0]["m"]["k1"]["v"],
            json!({"a": 1, "b": {"x": 1}}),
            "{out:#?}"
        );
    }
}
