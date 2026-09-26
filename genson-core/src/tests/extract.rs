// genson-core/src/tests/extract.rs
use super::*;
use serde_json::json;

fn spec() -> Vec<(String, String)> {
    vec![
        ("labels".to_string(), "id".to_string()),
        ("property-labels".to_string(), "property".to_string()),
    ]
}

fn rows(values: &[Value]) -> Vec<Option<String>> {
    values.iter().map(|v| Some(v.to_string())).collect()
}

fn parse(row: &Option<String>) -> Value {
    serde_json::from_str(row.as_deref().unwrap()).unwrap()
}

#[test]
fn test_removes_fields_at_any_depth_and_dedups() {
    let input = rows(&[
        json!({"P31": [{"mainsnak": {"property": "P31", "property-labels": {"en": "instance of"},
            "datavalue": {"id": "Q5", "labels": {"en": "human"}}}}]}),
        json!({"P31": [{"mainsnak": {"property": "P31", "property-labels": {"en": "instance of"},
            "datavalue": {"id": "Q5", "labels": {"en": "human"}}}}]}),
    ]);
    let out = extract_invariants(input, &spec()).unwrap();

    let slim = json!({"P31": [{"mainsnak": {"property": "P31", "datavalue": {"id": "Q5"}}}]});
    assert_eq!(parse(&out.rows[0]), slim);
    assert_eq!(parse(&out.rows[1]), slim);
    assert_eq!(
        out.lookup,
        vec![
            LookupEntry {
                field: "property-labels".into(),
                key: "P31".into(),
                value: json!({"en": "instance of"}),
            },
            LookupEntry {
                field: "labels".into(),
                key: "Q5".into(),
                value: json!({"en": "human"}),
            },
        ]
    );
}

#[test]
fn test_field_without_key_is_left_in_place() {
    let input = rows(&[json!({"labels": {"en": "x"}, "other": 1})]);
    let out = extract_invariants(input, &spec()).unwrap();
    assert_eq!(parse(&out.rows[0]), json!({"labels": {"en": "x"}, "other": 1}));
    assert!(out.lookup.is_empty());
}

#[test]
fn test_conflicting_subtrees_error() {
    let input = rows(&[
        json!({"id": "Q5", "labels": {"en": "human"}}),
        json!({"id": "Q5", "labels": {"en": "person"}}),
    ]);
    let err = extract_invariants(input, &spec()).unwrap_err();
    assert!(
        err.contains("field 'labels' is not invariant for its determinant 'id' ('Q5'"),
        "{err}"
    );
}

#[test]
fn test_null_and_invalid_rows_pass_through() {
    let input = vec![None, Some("not json".to_string())];
    let out = extract_invariants(input, &spec()).unwrap();
    assert_eq!(out.rows, vec![None, Some("not json".to_string())]);
}

#[test]
fn test_key_order_of_remaining_fields_is_kept() {
    let input = vec![Some(r#"{"b": 1, "labels": {"en": "x"}, "id": "Q1", "a": 2}"#.to_string())];
    let out = extract_invariants(input, &spec()).unwrap();
    assert_eq!(out.rows[0].as_deref(), Some(r#"{"b":1,"id":"Q1","a":2}"#));
}
