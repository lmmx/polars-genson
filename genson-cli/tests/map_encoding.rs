// tests/map_encoding.rs
use assert_cmd::Command;
use insta::assert_json_snapshot;
use serde_json::Value;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper: parse NDJSON CLI output into Vec<Value> for pretty JSON snapshots.
fn parse_ndjson(output: &str) -> Vec<Value> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("CLI should emit valid JSON"))
        .collect()
}

/// Snapshot for `--map-encoding mapping` (default Avro/JSON-style).
#[test]
fn test_map_encoding_mapping_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"id": "A", "labels": {{"en": "Hello", "fr": "Bonjour"}}}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0",
        "--map-encoding",
        "mapping",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    assert_json_snapshot!("cli_map_encoding_mapping", parse_ndjson(&stdout_str));
}

/// Snapshot for `--map-encoding entries` (list of single-entry objects).
#[test]
fn test_map_encoding_entries_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"id": "A", "labels": {{"en": "Hello", "fr": "Bonjour"}}}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0",
        "--map-encoding",
        "entries",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    assert_json_snapshot!("cli_map_encoding_entries", parse_ndjson(&stdout_str));
}

/// Snapshot for `--map-encoding kv` (list of {key,value} objects).
#[test]
fn test_map_encoding_kv_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"id": "A", "labels": {{"en": "Hello", "fr": "Bonjour"}}}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0",
        "--map-encoding",
        "kv",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    assert_json_snapshot!("cli_map_encoding_kv", parse_ndjson(&stdout_str));
}

#[test]
fn test_wrap_root_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"en": {{"language":"en","value":"Hello"}}, "fr": {{"language":"fr","value":"Bonjour"}}}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0",
        "--map-encoding",
        "kv",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    assert_json_snapshot!("cli_wrap_root", parse_ndjson(&stdout_str));
}
