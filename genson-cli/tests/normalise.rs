use assert_cmd::Command;
use insta::assert_snapshot;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_normalise_ndjson_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"id": "Q1", "aliases": [], "labels": {{}}, "description": "Example entity"}}"#
    )
    .unwrap();
    writeln!(
        temp,
        r#"{{"id": "Q2", "aliases": ["Sample"], "labels": {{"en": "Hello"}}, "description": null}}"#
    )
    .unwrap();
    writeln!(
        temp,
        r#"{{"id": "Q3", "aliases": null, "labels": {{"fr": "Bonjour"}}, "description": "Third one"}}"#
    ).unwrap();
    writeln!(
        temp,
        r#"{{"id": "Q4", "aliases": ["X","Y"], "labels": {{}}, "description": ""}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    assert_snapshot!("normalise_ndjson", stdout_str);
}

#[test]
fn test_normalise_union_coercion_snapshot() {
    // Create NDJSON file with heterogeneous types
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"int_field": 1, "float_field": 3.14, "bool_field": true}}"#
    )
    .unwrap();
    // These are strings but since the schema type is a union, the first type takes precedence and
    // the type coercion from string kicks in
    writeln!(
        temp,
        r#"{{"int_field": "42", "float_field": "2.718", "bool_field": "false"}}"#
    )
    .unwrap();
    writeln!(temp, r#"{{"int_field": null, "float_field": null}}"#).unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--coerce-strings",
        "--ndjson",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Compare against snapshot
    assert_snapshot!("normalise_union_coercion", stdout_str);
}
