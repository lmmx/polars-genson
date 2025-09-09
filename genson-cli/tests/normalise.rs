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

#[test]
fn test_normalise_string_or_array_snapshot() {
    // NDJSON with heterogeneous shapes for "foo"
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"foo": "json"}}"#).unwrap();
    writeln!(temp, r#"{{"foo": ["bar", "baz"]}}"#).unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot ensures scalar string widened to array, and arrays pass through
    assert_snapshot!("normalise_string_or_array", stdout_str);
}

#[test]
fn test_normalise_string_or_array_snapshot_rev() {
    // NDJSON with heterogeneous shapes for "foo"
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"foo": ["bar", "baz"]}}"#).unwrap();
    writeln!(temp, r#"{{"foo": "json"}}"#).unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot ensures scalar string widened to array, and arrays pass through
    assert_snapshot!("normalise_string_or_array_rev", stdout_str);
}

#[test]
fn test_normalise_object_or_array_snapshot() {
    // Create NDJSON file with mixed object/array values for the same field
    let mut temp = NamedTempFile::new().unwrap();
    // First row: array of objects
    writeln!(temp, r#"{{"foo": [{{"bar": 1}}]}}"#).unwrap();
    // Second row: single object
    writeln!(temp, r#"{{"foo": {{"bar": 2}}}}"#).unwrap();

    // Run CLI with normalisation enabled
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show that the single object was widened to an array
    assert_snapshot!("normalise_object_or_array", stdout_str);
}

#[test]
fn test_normalise_missing_field_snapshot() {
    // NDJSON with one row missing the "foo" field entirely
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"foo": "present"}}"#).unwrap();
    writeln!(temp, r#"{{"bar": 123}}"#).unwrap(); // foo missing here

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show that the second row has "foo": null
    assert_snapshot!("normalise_missing_field", stdout_str);
}

#[test]
fn test_normalise_null_vs_missing_field_snapshot() {
    // First row: foo explicitly null
    // Second row: foo completely missing
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"foo": null, "bar": 1}}"#).unwrap();
    writeln!(temp, r#"{{"bar": 2}}"#).unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show: row 1 foo=null, row 2 foo=null (injected for missing)
    assert_snapshot!("normalise_null_vs_missing_field", stdout_str);
}

#[test]
fn test_normalise_empty_map_snapshot() {
    // NDJSON where "labels" is always an empty map
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"id": "A", "labels": {{}}}}"#).unwrap();
    writeln!(temp, r#"{{"id": "B", "labels": {{"en": "Hello"}}}}"#).unwrap();

    // Run CLI with normalisation enabled (default empty_as_null=true)
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show labels normalised to null
    assert_snapshot!("normalise_empty_map", stdout_str);
}

#[test]
fn test_normalise_map_threshold_snapshot() {
    // NDJSON where "labels" vary but all values are strings
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"id": "A", "labels": {{"en": "Hello"}}}}"#).unwrap();
    writeln!(temp, r#"{{"id": "B", "labels": {{"fr": "Bonjour"}}}}"#).unwrap();

    // Use low map threshold to force rewriting into "map" type
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "2",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show "labels" stabilised as a map (additionalProperties form)
    assert_snapshot!("normalise_map_threshold", stdout_str);
}

#[test]
fn test_normalise_scalar_to_map_snapshot() {
    // NDJSON where "labels" is sometimes scalar
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, r#"{{"id": "A", "labels": "foo"}}"#).unwrap();
    writeln!(temp, r#"{{"id": "B", "labels": {{"en": "Hello"}}}}"#).unwrap();

    // Run CLI with normalisation enabled
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--normalise", "--ndjson", temp.path().to_str().unwrap()]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // Snapshot should show row 1 coerced into {"labels":{"default":"foo"}}
    assert_snapshot!("normalise_scalar_to_map", stdout_str);
}
