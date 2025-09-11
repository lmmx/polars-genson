use assert_cmd::Command;
use insta::{assert_snapshot, with_settings};
use serde_json::Value;
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
        stdout_str
    };

    // attach the original input rows as structured JSON in the header
    let input_json: Vec<Value> = rows
        .iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    with_settings!({
        info => &serde_json::json!({ "input": input_json })
    }, {
        assert_snapshot!(name, value);
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
        &[],
        false,
    );
}

#[test]
fn test_normalise_labels_map_of_structs_snapshot() {
    run_normalise_snapshot(
        "normalise_labels_map_of_structs",
        &[
            r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
            r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
        ],
        &["--map-encoding", "kv", "--map-threshold", "0"],
        true,
    );

    // Avro schema variant
    let rows = [
        r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
        r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
    ];
    run_avro_schema_snapshot(
        "avro_labels_map_of_structs",
        &rows,
        &["--map-encoding", "kv", "--map-threshold", "0"],
    );
}

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
            "2",
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
            "2",
            "--wrap-root",
            "labels",
        ],
    );
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

    with_settings!({
        info => &serde_json::json!({ "input": input_json })
    }, {
        assert_snapshot!(name, avro_str);
    });
}
