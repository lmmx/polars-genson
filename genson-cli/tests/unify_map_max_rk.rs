// genson-cli/tests/unify_map_max_rk.rs

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

/// Run genson-cli with unify-maps and specified map-max-rk
fn run_genson_with_rk(name: &str, rows: Vec<&str>, map_max_rk: &str, extra_args: &[&str]) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec![
        "--ndjson",
        "--map-threshold",
        "2",
        "--map-max-rk",
        map_max_rk,
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

/// Array-of-records test data with shared "index" field
fn array_of_records_rows() -> Vec<&'static str> {
    vec![
        r#"{"letters": {"a": [{"index": 0, "vowel": 0}]}}"#,
        r#"{"letters": {"b": [{"index": 1, "consonant": 0}]}}"#,
    ]
}

#[test]
fn test_array_unified_rk0_avro() {
    run_genson_with_rk(
        "array__unified__rk0__avro",
        array_of_records_rows(),
        "0",
        &["--avro"],
    );
}

#[test]
fn test_array_unified_rk1_avro() {
    run_genson_with_rk(
        "array__unified__rk1__avro",
        array_of_records_rows(),
        "1",
        &["--avro"],
    );
}

#[test]
fn test_array_unified_rk2_avro() {
    run_genson_with_rk(
        "array__unified__rk2__avro",
        array_of_records_rows(),
        "2",
        &["--avro"],
    );
}
