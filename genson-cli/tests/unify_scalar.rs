// genson-cli/tests/unify_scalar.rs

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

/// Run genson-cli with scalar unification settings
fn run_genson_scalar_unify(name: &str, rows: Vec<&str>, extra_args: &[&str]) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--ndjson", "--map-threshold", "1", "--unify-maps"];
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

/// Colors with hex string map test data
/// Creates nested structure: colors (map) -> records -> hex (map) -> strings
/// The missing hex field in row 3 triggers scalar unification
fn hex_string_rows() -> Vec<&'static str> {
    vec![
        r#"{"colors": {"a": {"hex": {"r": "ff"}}, "b": {"hex": {"g": "00"}}}}"#,
        r#"{"colors": {"c": {"hex": {"b": "cc"}}, "a": {"hex": {"g": "aa"}}}}"#,
        r#"{"colors": {"b": {"rgb": 255}}}"#,
    ]
}

/// Colors with rgb integer map test data  
/// Creates nested structure: colors (map) -> records -> rgb (map) -> integers
/// The missing rgb field in row 3 triggers scalar unification
fn rgb_integer_rows() -> Vec<&'static str> {
    vec![
        r#"{"colors": {"a": {"rgb": {"r": 255}}, "b": {"rgb": {"g": 128}}}}"#,
        r#"{"colors": {"c": {"rgb": {"b": 64}}, "a": {"rgb": {"g": 200}}}}"#,
        r#"{"colors": {"b": {"hex": "00ff00"}}}"#,
    ]
}

// Test hex string scalar unification in different output formats

#[test]
fn test_hex_string_scalar_unify_avro() {
    run_genson_scalar_unify(
        "hex_string__scalar_unify__avro",
        hex_string_rows(),
        &["--avro"],
    );
}

#[test]
fn test_hex_string_scalar_unify_normalize() {
    run_genson_scalar_unify(
        "hex_string__scalar_unify__normalize",
        hex_string_rows(),
        &["--normalise"],
    );
}

#[test]
fn test_hex_string_scalar_unify_json() {
    run_genson_scalar_unify("hex_string__scalar_unify__json", hex_string_rows(), &[]);
}

// Test rgb integer scalar unification in different output formats

#[test]
fn test_rgb_integer_scalar_unify_avro() {
    run_genson_scalar_unify(
        "rgb_integer__scalar_unify__avro",
        rgb_integer_rows(),
        &["--avro"],
    );
}

#[test]
fn test_rgb_integer_scalar_unify_normalize() {
    run_genson_scalar_unify(
        "rgb_integer__scalar_unify__normalize",
        rgb_integer_rows(),
        &["--normalise"],
    );
}

#[test]
fn test_rgb_integer_scalar_unify_json() {
    run_genson_scalar_unify("rgb_integer__scalar_unify__json", rgb_integer_rows(), &[]);
}
