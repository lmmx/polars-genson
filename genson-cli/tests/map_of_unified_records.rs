use assert_cmd::Command;
use insta::{assert_snapshot, with_settings};
use serde_json::Value;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper: write lines of NDJSON to a temp file
fn write_ndjson(rows: &[&str]) -> NamedTempFile {
    let mut temp = NamedTempFile::new().unwrap();
    for row in rows {
        writeln!(temp, "{}", row).unwrap();
    }
    temp
}

/// Letter frequency data with vowel/consonant record variants
fn letter_frequency_rows() -> Vec<&'static str> {
    vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}"#,
        r#"{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}"#,
        r#"{"letter": {"c": {"alphabet": 2, "consonant": 1, "frequency": 0.0278}}}"#,
        r#"{"letter": {"d": {"alphabet": 3, "consonant": 2, "frequency": 0.0425}}}"#,
        r#"{"letter": {"e": {"alphabet": 4, "vowel": 4, "frequency": 0.1270}}}"#,
    ]
}

/// Incompatible rows: minimal 2 rows with conflicting `alphabet` types
fn incompatible_rows() -> Vec<&'static str> {
    vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}"#, // int
        r#"{"letter": {"b": {"alphabet": "one", "consonant": 0, "frequency": 0.0150}}}"#, // string
    ]
}

/// Check if the current output matches the verified/blessed version
fn is_output_approved(snapshot_name: &str, output: &str) -> bool {
    let module_file = file!();
    let module_stem = std::path::Path::new(module_file)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let verified_path = format!("tests/verified/{}__{}.snap", module_stem, snapshot_name);

    if let Ok(verified_content) = fs::read_to_string(&verified_path) {
        if let Some(header_end) = verified_content.find("\n---\n") {
            let verified_output = &verified_content[header_end + 5..];
            return verified_output.trim() == output.trim();
        }
    }
    false
}

/// Run genson-cli with unify-maps and given extra args
fn run_genson_unified(name: &str, rows: Vec<&str>, extra_args: &[&str]) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--ndjson", "--map-threshold", "5", "--unify-maps"];
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    let args_for_metadata = args.clone();
    cmd.args(args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    let input_json: Vec<Value> = rows
        .into_iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    let approved = is_output_approved(name, &stdout_str);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args_for_metadata[..args_for_metadata.len()-1],
            "input": input_json
        })
    }, {
        assert_snapshot!(name, stdout_str);
    });
}

#[test]
fn test_unified_maps_jsonschema() {
    run_genson_unified("unified__jsonschema", letter_frequency_rows(), &[]);
}

#[test]
fn test_unified_maps_avro() {
    run_genson_unified("unified__avro", letter_frequency_rows(), &["--avro"]);
}

#[test]
fn test_unified_maps_normalize() {
    run_genson_unified(
        "unified__normalize",
        letter_frequency_rows(),
        &["--normalise"],
    );
}

#[test]
fn test_incompatible_maps_jsonschema() {
    run_genson_unified("incompatible__jsonschema", incompatible_rows(), &[]);
}

#[test]
fn test_incompatible_maps_avro() {
    run_genson_unified("incompatible__avro", incompatible_rows(), &["--avro"]);
}

#[test]
fn test_incompatible_maps_normalize() {
    run_genson_unified(
        "incompatible__normalize",
        incompatible_rows(),
        &["--normalise"],
    );
}
