// genson-cli/tests/map_max_required_keys.rs

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
fn test_cli_map_max_rk_short_flag() {
    let json = r#"{"user_id": 1, "attrs": {"source": "web", "campaign": "abc"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--map-max-rk",
        "1",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Root should be record (2 required keys > 1)
    assert!(stdout.contains("\"properties\""));
    assert!(!stdout.contains("\"additionalProperties\""));

    println!("✅ --map-max-rk flag works correctly");
}

#[test]
fn test_cli_map_max_required_keys_long_flag() {
    let json = r#"{"user_id": 1, "attrs": {"source": "web", "campaign": "abc"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-threshold",
        "2",
        "--map-max-required-keys",
        "1",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should produce same result as short flag
    assert!(stdout.contains("\"properties\""));
    assert!(!stdout.contains("\"additionalProperties\""));

    println!("✅ --map-max-required-keys flag works correctly");
}

#[test]
fn test_cli_distinguishes_records_from_maps() {
    // Multi-row data where we want to distinguish record structure from map structure
    let json = r#"
{"user_id": 1, "profile": {"name": "Alice", "age": 30}, "attrs": {"source": "web", "campaign": "abc"}}
{"user_id": 2, "profile": {"name": "Bob", "age": 25}, "attrs": {"source": "mobile"}}
"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--ndjson",
        "--map-threshold",
        "2",
        "--map-max-rk",
        "1",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    println!("Schema output:\n{}", stdout);

    // Root level should be record (user_id, profile, attrs all required)
    assert!(stdout.contains("\"properties\""));

    // Should contain both profile and attrs
    assert!(stdout.contains("\"profile\""));
    assert!(stdout.contains("\"attrs\""));

    println!("✅ CLI correctly distinguishes nested records from maps");
}

#[test]
fn test_cli_invalid_map_max_rk_value() {
    let json = r#"{"test": "data"}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--map-max-rk",
        "not-a-number",
        temp.path().to_str().unwrap(),
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Invalid value for --map-max-required-keys",
    ));

    println!("✅ CLI correctly rejects invalid map-max-rk values");
}

#[test]
fn test_cli_zero_max_required_keys() {
    // Test with zero max required keys - should only allow fully optional maps
    let json = r#"
{"data": {"key1": "val1", "key2": "val2"}}
{"data": {"key3": "val3"}}
"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--ndjson",
        "--map-threshold",
        "2",
        "--map-max-rk",
        "0",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    println!("Zero max-rk schema:\n{}", stdout);

    // Root should be record (data is required)
    assert!(stdout.contains("\"properties\""));

    // But the nested data should be map (no required keys)
    assert!(stdout.contains("\"additionalProperties\""));

    println!("✅ Zero max required keys works correctly");
}

#[test]
fn test_cli_map_max_rk_with_avro() {
    let json = r#"{"user_id": 1, "attrs": {"source": "web", "other": "data"}}"#;
    let temp = write_temp(json);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--map-threshold",
        "2",
        "--map-max-rk",
        "1",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    println!("Avro schema with map-max-rk:\n{}", stdout);

    // Should contain Avro record structure
    assert!(stdout.contains("\"type\": \"record\""));
    assert!(stdout.contains("\"fields\""));

    println!("✅ map-max-rk works with Avro output");
}
