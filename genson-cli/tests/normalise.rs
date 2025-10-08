use assert_cmd::Command;
use insta::{assert_snapshot, with_settings};
use serde_json::Value;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

fn pretty_print_ndjson(raw: &str) -> String {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let val: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|_| panic!("invalid JSON line: {}", line));
            serde_json::to_string_pretty(&val).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Check if the current output matches the verified/blessed version
fn is_output_approved(snapshot_name: &str, output: &str) -> bool {
    // file!() gives something like "tests/normalise.rs"
    let module_file = file!();
    let module_stem = std::path::Path::new(module_file)
        .file_stem()
        .unwrap()
        .to_string_lossy();

    let verified_path = format!("tests/verified/{}__{}.snap", module_stem, snapshot_name);

    if let Ok(verified_content) = fs::read_to_string(&verified_path) {
        // Extract just the content part from the verified snapshot
        // Skip the YAML header (everything up to and including the "---" line)
        if let Some(header_end) = verified_content.find("\n---\n") {
            let verified_output = &verified_content[header_end + 5..]; // Skip "\n---\n"
            return verified_output.trim() == output.trim();
        }
    }
    false
}

/// Run genson-cli with given args, writing `rows` into a temp NDJSON file,
/// and snapshot the CLI output with the input data included in the header.
fn run_normalise_snapshot(name: &str, rows: &[&str], extra_args: &[&str], pretty: bool) {
    // write rows to a temp NDJSON file
    let mut temp = NamedTempFile::new().unwrap();
    for row in rows {
        writeln!(temp, "{}", row).unwrap();
    }

    // run the CLI
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--normalise", "--ndjson"];
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    // optionally pretty-print NDJSON
    let value = if pretty {
        pretty_print_ndjson(&stdout_str)
    } else {
        stdout_str.clone()
    };

    // attach the original input rows as structured JSON in the header
    let input_json: Vec<Value> = rows
        .iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    // Check if this output matches the blessed/verified version
    let approved = is_output_approved(name, &value);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args[..args.len()-1],  // exclude the temp file path
            "input": input_json
        })
    }, {
        assert_snapshot!(name, value);
    });
}

/// Simple helper for Avro-only schema snapshots
fn run_avro_schema_snapshot(name: &str, rows: &[&str], extra_args: &[&str]) {
    let mut temp = NamedTempFile::new().unwrap();
    for row in rows {
        writeln!(temp, "{}", row).unwrap();
    }

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--avro", "--ndjson"];
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(output).unwrap();

    let input_json: Vec<Value> = rows
        .iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    // Check if this output matches the blessed/verified version
    let approved = is_output_approved(name, &avro_str);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": &args[..&args.len()-1],  // exclude the temp file path
            "input": input_json
        })
    }, {
        assert_snapshot!(name, avro_str);
    });
}

#[test]
fn test_normalise_ndjson_snapshot() {
    run_normalise_snapshot(
        "normalise_ndjson",
        &[
            r#"{"id": "Q1", "aliases": [], "labels": {}, "description": "Example entity"}"#,
            r#"{"id": "Q2", "aliases": ["Sample"], "labels": {"en": "Hello"}, "description": null}"#,
            r#"{"id": "Q3", "aliases": null, "labels": {"fr": "Bonjour"}, "description": "Third one"}"#,
            r#"{"id": "Q4", "aliases": ["X","Y"], "labels": {}, "description": ""}"#,
        ],
        &[],
        false,
    );
}

#[test]
fn test_normalise_union_coercion_snapshot() {
    run_normalise_snapshot(
        "normalise_union_coercion",
        &[
            r#"{"int_field": 1, "float_field": 3.14, "bool_field": true}"#,
            r#"{"int_field": "42", "float_field": "2.718", "bool_field": "false"}"#,
            r#"{"int_field": null, "float_field": null}"#,
        ],
        &["--coerce-strings"],
        false,
    );
}

#[test]
fn test_normalise_string_or_array_snapshot() {
    run_normalise_snapshot(
        "normalise_string_or_array",
        &[r#"{"foo": "json"}"#, r#"{"foo": ["bar", "baz"]}"#],
        &[],
        false,
    );
}

#[test]
fn test_normalise_string_or_array_snapshot_rev() {
    run_normalise_snapshot(
        "normalise_string_or_array_rev",
        &[r#"{"foo": ["bar", "baz"]}"#, r#"{"foo": "json"}"#],
        &[],
        false,
    );
}

#[test]
fn test_normalise_object_or_array_snapshot() {
    run_normalise_snapshot(
        "normalise_object_or_array",
        &[r#"{"foo": [{"bar": 1}]}"#, r#"{"foo": {"bar": 2}}"#],
        &[],
        false,
    );
}

#[test]
fn test_normalise_missing_field_snapshot() {
    run_normalise_snapshot(
        "normalise_missing_field",
        &[r#"{"foo": "present"}"#, r#"{"bar": 123}"#],
        &[],
        false,
    );
}

#[test]
fn test_normalise_null_vs_missing_field_snapshot() {
    run_normalise_snapshot(
        "normalise_null_vs_missing_field",
        &[r#"{"foo": null, "bar": 1}"#, r#"{"bar": 2}"#],
        &[],
        false,
    );
}

#[test]
fn test_normalise_empty_map_snapshot() {
    run_normalise_snapshot(
        "normalise_empty_map",
        &[
            r#"{"id": "A", "labels": {}}"#,
            r#"{"id": "B", "labels": {"en": "Hello"}}"#,
        ],
        &["--map-threshold", "0"],
        false,
    );
}

#[test]
fn test_normalise_map_threshold_snapshot() {
    run_normalise_snapshot(
        "normalise_map_threshold",
        &[
            r#"{"id": "A", "labels": {"en": "Hello"}}"#,
            r#"{"id": "B", "labels": {"fr": "Bonjour"}}"#,
        ],
        &["--map-threshold", "2"],
        false,
    );
}

#[test]
fn test_normalise_scalar_to_map_snapshot() {
    run_normalise_snapshot(
        "normalise_scalar_to_map",
        &[
            r#"{"id": "A", "labels": "foo"}"#,
            r#"{"id": "B", "labels": {"en": "Hello"}}"#,
        ],
        &["--map-threshold", "1"],
        false,
    );
}

/// Both schema and normalisation give `{"document": null}`, unclear why (threshold invariant)
#[test]
fn test_normalise_labels_map_of_structs_snapshot() {
    // The following DO NOT WORK if provided the content of the labels field at the top level
    run_normalise_snapshot(
        "normalise_labels_map_of_structs",
        &[
            r#"{"labels":{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}}"#,
            r#"{"labels":{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}}"#,
        ],
        &["--map-encoding", "kv", "--map-threshold", "3"],
        true,
    );

    // Avro schema variant
    run_avro_schema_snapshot(
        "avro_labels_map_of_structs",
        &[
            r#"{"labels":{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}}"#,
            r#"{"labels":{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}}"#,
        ],
        &["--map-encoding", "kv", "--map-threshold", "3"],
    );
}

/// A map of structs is a Map<key:String, value:<Record{language:String,Value:String}>>
/// The key and value should be promoted to "key": ..., "value": ... in the kv map encoding
///
/// - The inner record should be schematised to Record
/// - The outer map should be promoted to kv map encoding
#[test]
fn test_normalise_labels_wrap_map_of_structs_snapshot() {
    run_normalise_snapshot(
        "normalise_labels_wrap_map_of_structs",
        &[
            r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
            r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
        ],
        &[
            "--map-encoding",
            "kv",
            "--map-threshold",
            "3",
            "--wrap-root",
            "labels",
        ],
        true,
    );

    // Avro schema variant
    let rows = [
        r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
        r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
    ];
    run_avro_schema_snapshot(
        "avro_labels_wrap_map_of_structs",
        &rows,
        &[
            "--map-encoding",
            "kv",
            "--map-threshold",
            "3",
            "--wrap-root",
            "labels",
        ],
    );
}

#[test]
fn test_normalise_force_scalar_promotion_snapshot() {
    run_normalise_snapshot(
        "normalise_force_scalar_promotion",
        &[r#"{"precision": 11}"#],
        &["--force-scalar-promotion", "precision"],
        true,
    );
}
