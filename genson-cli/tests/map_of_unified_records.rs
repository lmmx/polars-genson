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

/// Run genson-cli with unify-maps and given extra args
fn run_genson_unified(name: &str, rows: Vec<&str>, extra_args: &[&str]) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec![
        "--ndjson",
        "--map-threshold",
        "2",
        "--map-max-rk",
        "1",
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

// Tests for unify map of records

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

/// Disjoint rows: minimal 2 rows with non-overlapping fields
fn disjoint_rows() -> Vec<&'static str> {
    vec![
        r#"{"letter": {"a": {"vowel": 0, "a_for": "apple"}}}"#,
        r#"{"letter": {"b": {"consonant": 0, "b_for": "byte"}}}"#,
    ]
}

/// Incompatible rows: minimal 2 rows with conflicting `alphabet` types
fn incompatible_rows() -> Vec<&'static str> {
    vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}"#, // int
        r#"{"letter": {"b": {"alphabet": "one", "consonant": 0, "frequency": 0.0150}}}"#, // string
    ]
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

#[test]
fn test_disjoint_maps_jsonschema() {
    run_genson_unified("disjoint__jsonschema", disjoint_rows(), &[]);
}

#[test]
fn test_disjoint_maps_avro() {
    run_genson_unified("disjoint__avro", disjoint_rows(), &["--avro"]);
}

#[test]
fn test_disjoint_maps_normalize() {
    run_genson_unified("disjoint__normalize", disjoint_rows(), &["--normalise"]);
}

// Tests for unify map of array of records

/// Minimal array-of-records rows: vowel vs consonant variants
fn array_of_records_rows() -> Vec<&'static str> {
    vec![
        r#"{"letters": {"a": [{"index": 0, "vowel": 0}]}}"#,
        r#"{"letters": {"b": [{"index": 1, "consonant": 0}]}}"#,
    ]
}

/// Incompatible array-of-records rows: conflict on index type (int vs string)
fn array_of_records_incompatible_rows() -> Vec<&'static str> {
    vec![
        r#"{"letters": {"a": [{"index": 0, "vowel": 0}]}}"#, // int index
        r#"{"letters": {"b": [{"index": "one", "consonant": 0}]}}"#, // string index
    ]
}

#[test]
fn test_array_unified_jsonschema() {
    run_genson_unified("array__unified__jsonschema", array_of_records_rows(), &[]);
}

#[test]
fn test_array_unified_avro() {
    run_genson_unified("array__unified__avro", array_of_records_rows(), &["--avro"]);
}

#[test]
fn test_array_unified_normalize() {
    run_genson_unified(
        "array__unified__normalize",
        array_of_records_rows(),
        &["--normalise"],
    );
}

#[test]
fn test_array_incompatible_jsonschema() {
    run_genson_unified(
        "array__incompatible__jsonschema",
        array_of_records_incompatible_rows(),
        &[],
    );
}

#[test]
fn test_array_incompatible_avro() {
    run_genson_unified(
        "array__incompatible__avro",
        array_of_records_incompatible_rows(),
        &["--avro"],
    );
}

#[test]
fn test_array_incompatible_normalize() {
    run_genson_unified(
        "array__incompatible__normalize",
        array_of_records_incompatible_rows(),
        &["--normalise"],
    );
}

/// Minimal array-of-records rows with mismatched nested `value` objects.
/// "a" has `value.vowel` (boolean), "b" has `value.cap` (string).
fn array_of_records_value_rows() -> Vec<&'static str> {
    vec![
        r#"{"letters": {"a": [{"index": 0, "value": {"vowel": true}}]}}"#,
        r#"{"letters": {"b": [{"index": 1, "value": {"cap": "B"}}]}}"#,
    ]
}

#[test]
fn test_array_value_unified_jsonschema() {
    run_genson_unified(
        "array__value__unified__jsonschema",
        array_of_records_value_rows(),
        &[],
    );
}

#[test]
fn test_array_value_unified_avro() {
    run_genson_unified(
        "array__value__unified__avro",
        array_of_records_value_rows(),
        &["--avro"],
    );
}

#[test]
fn test_array_value_unified_normalize() {
    run_genson_unified(
        "array__value__unified__normalize",
        array_of_records_value_rows(),
        &["--normalise"],
    );
}

/// Minimal array-of-records rows with scalar vs object `value`.
/// "a" has `value` as object with {id, labels}, "b" has `value` as string.
fn array_of_records_scalar_object_rows() -> Vec<&'static str> {
    vec![
        r#"{"letters": {"a": [{"index": 0, "value": {"id": "X", "labels": {"en": "thing"}}}]}}"#,
        r#"{"letters": {"b": [{"index": 1, "value": "scalar-string"}]}}"#,
    ]
}

#[test]
fn test_array_scalar_object_unified_jsonschema() {
    run_genson_unified(
        "array__scalar_object__unified__jsonschema",
        array_of_records_scalar_object_rows(),
        &[],
    );
}

#[test]
fn test_array_scalar_object_unified_avro() {
    run_genson_unified(
        "array__scalar_object__unified__avro",
        array_of_records_scalar_object_rows(),
        &["--avro"],
    );
}

#[test]
fn test_array_scalar_object_unified_normalize() {
    run_genson_unified(
        "array__scalar_object__unified__normalize",
        array_of_records_scalar_object_rows(),
        &["--normalise"],
    );
}

/// Run genson-cli with claims fixture using single JSON file (not NDJSON)
fn run_genson_claims_fixture(name: &str, extra_args: &[&str]) {
    let fixture_content = include_str!("data/claims_fixture.json");
    let temp = write_json_file(fixture_content);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--map-threshold", "2", "--unify-maps"];
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    let args_for_metadata = args.clone();
    cmd.args(args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    let input_json: Value = serde_json::from_str(fixture_content).unwrap();

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

/// Helper: write JSON content to a temp file (not NDJSON)
fn write_json_file(content: &str) -> NamedTempFile {
    let mut temp = NamedTempFile::new().unwrap();
    write!(temp, "{}", content).unwrap();
    temp
}

#[test]
fn test_claims_fixture_unified_jsonschema() {
    run_genson_claims_fixture("claims_fixture__unified__jsonschema", &[]);
}

#[test]
fn test_claims_fixture_unified_avro() {
    run_genson_claims_fixture("claims_fixture__unified__avro", &["--avro"]);
}

#[test]
fn test_claims_fixture_unified_normalize() {
    run_genson_claims_fixture("claims_fixture__unified__normalize", &["--normalise"]);
}
