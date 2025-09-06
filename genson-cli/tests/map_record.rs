// genson-cli/tests/map_record.rs
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn write_temp(json: &str) -> NamedTempFile {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(json.as_bytes())
        .expect("Failed to write to temp file");
    temp_file
}

#[test]
fn test_map_threshold_flag_rewrites_to_map() {
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour", "de": "Hallo"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--map-threshold", "2", temp.path().to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"labels\""))
        .stdout(predicate::str::contains("\"additionalProperties\""))
        .stdout(predicate::str::contains("\"properties\"").not());
}

#[test]
fn test_map_threshold_default_keeps_record() {
    // 3 keys is less than the default threshold of 20, so this stays a record
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour", "de": "Hallo"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.arg(temp.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"labels\""))
        .stdout(predicate::str::contains("\"properties\""))
        .stdout(predicate::str::contains("\"additionalProperties\"").not());
}

#[test]
fn test_force_type_map() {
    // Normally below threshold → record, but override should force map
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--force-type", "labels:map", temp.path().to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"labels\""))
        .stdout(predicate::str::contains("\"additionalProperties\""))
        .stdout(predicate::str::contains("\"properties\"").not());
}

#[test]
fn test_force_type_record() {
    // Above threshold, would normally rewrite → map, but override should force record
    let json = r#"{"labels": {"a": "x", "b": "y", "c": "z"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--force-type",
        "labels:record",
        temp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"labels\""))
        .stdout(predicate::str::contains("\"properties\""))
        .stdout(predicate::str::contains("\"additionalProperties\"").not());
}

#[test]
fn test_force_type_multiple_fields() {
    let json = r#"{
        "labels": {"en": "Hello", "fr": "Bonjour"},
        "claims": {"x": "foo", "y": "bar", "z": "baz"}
    }"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--force-type",
        "labels:map,claims:record",
        temp.path().to_str().unwrap(),
    ]);
    let output = cmd.assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // labels forced → map
    assert!(stdout.contains("\"labels\""));
    assert!(stdout.contains("\"additionalProperties\""));

    // claims forced → record
    assert!(stdout.contains("\"claims\""));
    assert!(stdout.contains("\"properties\""));
    assert!(!stdout.contains("\"claims\": {\"type\": \"object\", \"additionalProperties\""));
}
