// tests/map_encoding.rs
use assert_cmd::Command;
use insta::{assert_json_snapshot, with_settings};
use serde_json::Value;
use std::fs;
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

/// Check if the current output matches the verified/blessed version
fn is_output_approved(snapshot_name: &str, output: &[Value]) -> bool {
    let module_file = file!();
    let module_stem = std::path::Path::new(module_file)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let verified_path = format!("tests/verified/{}__{}.snap", module_stem, snapshot_name);

    if let Ok(verified_content) = fs::read_to_string(&verified_path) {
        if let Some(header_end) = verified_content.find("\n---\n") {
            let verified_output = &verified_content[header_end + 5..];
            if let Ok(verified_json) = serde_json::from_str::<Vec<Value>>(verified_output.trim()) {
                return verified_json == *output;
            }
        }
    }
    false
}

/// Run genson-cli with given args and snapshot the JSON output.
/// Always attaches approval status as metadata.
fn run_map_encoding_test(snapshot_name: &str, input_json: &str, extra_args: &[&str]) {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "{}", input_json).unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--normalise", "--ndjson", "--map-threshold", "0"];
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    let args_for_metadata = args.clone();
    cmd.args(args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();
    let parsed_output = parse_ndjson(&stdout_str);

    // Parse input for metadata
    let input_value: Value = serde_json::from_str(input_json).expect("Input should be valid JSON");

    // Check if this output matches the blessed/verified version
    let approved = is_output_approved(snapshot_name, &parsed_output);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args_for_metadata[..args_for_metadata.len()-1], // exclude temp file path
            "input": input_value
        })
    }, {
        assert_json_snapshot!(snapshot_name, parsed_output);
    });
}

/// Snapshot for `--map-encoding mapping` (default Avro/JSON-style).
#[test]
fn test_map_encoding_mapping_snapshot() {
    run_map_encoding_test(
        "cli_map_encoding_mapping",
        r#"{"id": "A", "labels": {"en": "Hello", "fr": "Bonjour"}}"#,
        &["--map-encoding", "mapping"],
    );
}

/// Snapshot for `--map-encoding entries` (list of single-entry objects).
#[test]
fn test_map_encoding_entries_snapshot() {
    run_map_encoding_test(
        "cli_map_encoding_entries",
        r#"{"id": "A", "labels": {"en": "Hello", "fr": "Bonjour"}}"#,
        &["--map-encoding", "entries"],
    );
}

/// Snapshot for `--map-encoding kv` (list of {key,value} objects).
#[test]
fn test_map_encoding_kv_snapshot() {
    run_map_encoding_test(
        "cli_map_encoding_kv",
        r#"{"id": "A", "labels": {"en": "Hello", "fr": "Bonjour"}}"#,
        &["--map-encoding", "kv"],
    );
}

#[test]
fn test_wrap_root_snapshot() {
    run_map_encoding_test(
        "cli_wrap_root",
        r#"{"en": {"language":"en","value":"Hello"}, "fr": {"language":"fr","value":"Bonjour"}}"#,
        &[
            "--map-threshold",
            "2",
            "--map-encoding",
            "kv",
            "--wrap-root",
            "labels",
        ],
    );
}
