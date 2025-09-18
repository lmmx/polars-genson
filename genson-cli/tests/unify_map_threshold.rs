// genson-cli/tests/unify_map_threshold.rs

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

/// Run genson-cli with unify-maps and specified map-threshold
fn run_genson_with_threshold(
    name: &str,
    rows: Vec<&str>,
    map_threshold: &str,
    extra_args: &[&str],
) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec![
        "--ndjson",
        "--map-threshold",
        map_threshold,
        "--map-max-rk",
        "2", // Allows inner records (1 required ≤ 2) to become maps if threshold allows
        "--unify-maps",
    ];
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

/// Test data with alphabet ranges and letter records.
///
/// Structure:
/// - Root: 3 required keys (from, to, letter) > map-max-rk=2 → always stays record
/// - letter: 0 required keys (a,b,c,d all optional) ≤ map-max-rk=2 → always becomes map
/// - Inner records: 1 required key (index), 2 optional (vowel, consonant), 3 total fields
///   - With map-max-rk=2: 1 required ≤ 2 → can become maps if threshold allows
///
/// Expected threshold behavior:
/// - threshold=2,3: Inner records have 3 fields ≥ threshold → become individual maps (no unification)  
/// - threshold=4: Inner records have 3 fields < 4 → stay records → get unified
fn threshold_test_rows() -> Vec<&'static str> {
    vec![
        r#"{"from": 0, "to": 1, "letter": {"a": {"index": 0, "vowel": 0}, "b": {"index": 1, "consonant": 0}}}"#,
        r#"{"from": 2, "to": 3, "letter": {"c": {"index": 2, "consonant": 1}, "d": {"index": 3, "consonant": 2}}}"#,
    ]
}

// Test progression: threshold 2, 3, 4
// Expected behavior:
// - threshold=2: Inner records (3 fields ≥ 2) become maps → result: map of maps (no unification)
// - threshold=3: Inner records (3 fields ≥ 3) become maps → result: map of maps (same as threshold=2)
// - threshold=4: Inner records (3 fields < 4) stay records → result: map of unified records

#[test]
fn test_unified_threshold2_avro() {
    run_genson_with_threshold(
        "unified__threshold2__avro",
        threshold_test_rows(),
        "2",
        &["--avro"],
    );
}

#[test]
fn test_unified_threshold2_normalize() {
    run_genson_with_threshold(
        "unified__threshold2__normalize",
        threshold_test_rows(),
        "2",
        &["--normalise"],
    );
}

#[test]
fn test_unified_threshold3_avro() {
    run_genson_with_threshold(
        "unified__threshold3__avro",
        threshold_test_rows(),
        "3",
        &["--avro"],
    );
}

#[test]
fn test_unified_threshold3_normalize() {
    run_genson_with_threshold(
        "unified__threshold3__normalize",
        threshold_test_rows(),
        "3",
        &["--normalise"],
    );
}

#[test]
fn test_unified_threshold4_avro() {
    run_genson_with_threshold(
        "unified__threshold4__avro",
        threshold_test_rows(),
        "4",
        &["--avro"],
    );
}

#[test]
fn test_unified_threshold4_normalize() {
    run_genson_with_threshold(
        "unified__threshold4__normalize",
        threshold_test_rows(),
        "4",
        &["--normalise"],
    );
}
