// genson-cli/tests/map_of_records.rs
use assert_cmd::Command;
use insta::{assert_snapshot, with_settings};
use serde_json::Value;
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

/// Prototypical input: map<string, record{language, value}>
fn sample_rows() -> Vec<&'static str> {
    vec![
        r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
        r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
    ]
}

/// Run genson-cli with given mode/args and snapshot the output.
/// Always attaches the original input JSON as metadata.
fn run_genson(mode: &str, name: &str, extra_args: &[&str]) {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--ndjson", "--map-threshold", "0"];
    if mode == "avro" {
        args.insert(0, "--avro");
    }
    args.extend_from_slice(extra_args);
    args.push(temp.path().to_str().unwrap());
    cmd.args(args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    let input_json: Vec<Value> = sample_rows()
        .into_iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    with_settings!({
        info => &serde_json::json!({ "input": input_json })
    }, {
        assert_snapshot!(name, stdout_str);
    });
}

#[test]
fn test_map_of_records_infer_jsonschema() {
    run_genson("jsonschema", "infer__jsonschema", &[]);
}

#[test]
fn test_map_of_records_infer_avro() {
    run_genson("avro", "infer__avro", &[]);
}

#[test]
fn test_map_of_records_infer_jsonschema_wrapped() {
    run_genson(
        "jsonschema",
        "infer__jsonschema_wrap",
        &["--wrap-root", "labels"],
    );
}

#[test]
fn test_map_of_records_infer_avro_wrapped() {
    run_genson("avro", "infer__avro_wrap", &["--wrap-root", "labels"]);
}

#[test]
fn test_map_of_records_infer_jsonschema_kv() {
    run_genson(
        "jsonschema",
        "infer__jsonschema_kv",
        &["--map-encoding", "kv"],
    );
}

#[test]
fn test_map_of_records_infer_avro_kv() {
    run_genson("avro", "infer__avro_kv", &["--map-encoding", "kv"]);
}

#[test]
fn test_map_of_records_infer_jsonschema_wrapped_kv() {
    run_genson(
        "jsonschema",
        "infer__jsonschema_wrap_kv",
        &["--map-encoding", "kv", "--wrap-root", "labels"],
    );
}

#[test]
fn test_map_of_records_infer_avro_wrapped_kv() {
    run_genson(
        "avro",
        "infer__avro_wrap_kv",
        &["--map-encoding", "kv", "--wrap-root", "labels"],
    );
}
