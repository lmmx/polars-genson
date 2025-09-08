// genson-cli/tests/map_record_avro.rs
// These tests require: cargo test --features avro
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
fn test_map_threshold_rewrites_to_avro_map() {
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour", "de": "Hallo"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--avro",
        temp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        // Avro schema should explicitly have a map type
        .stdout(predicate::str::contains(r#""type": "map""#))
        // And the values of that map should be strings
        .stdout(predicate::str::contains(r#""values": "string""#));
}

#[test]
fn test_default_threshold_keeps_avro_record() {
    // Three keys < default threshold (20), stays as record
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour", "de": "Hallo"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(["--avro", temp.path().to_str().unwrap()]);
    cmd.assert()
        .success()
        // Record should appear as type=record with fields
        .stdout(predicate::str::contains(r#""type": "record""#))
        .stdout(predicate::str::contains(r#""fields""#))
        .stdout(predicate::str::contains(r#""name": "labels""#));
}

#[ignore]
#[test]
fn test_force_type_map_in_avro() {
    let json = r#"{"labels": {"en": "Hello", "fr": "Bonjour"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--force-type",
        "labels:map",
        "--avro",
        temp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""type": "map""#))
        .stdout(predicate::str::contains(r#""values": "string""#))
        .stdout(predicate::str::contains(r#""fields""#).not());
}

#[ignore]
#[test]
fn test_force_type_record_in_avro() {
    // Above threshold → map by default, but override → record
    let json = r#"{"labels": {"a": "x", "b": "y", "c": "z"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--force-type",
        "labels:record",
        "--avro",
        temp.path().to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""type": "record""#))
        .stdout(predicate::str::contains(r#""fields""#))
        .stdout(predicate::str::contains(r#""type": "map""#).not());
}
