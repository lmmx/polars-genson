// genson-cli/tests/map_of_records.rs
use assert_cmd::Command;
use insta::assert_snapshot;
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

#[test]
fn test_map_of_records_infer_jsonschema() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--ndjson",
        "--map-threshold",
        "0",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__jsonschema", stdout_str);
}

#[test]
fn test_map_of_records_infer_avro() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0", // force map detection
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__avro", avro_str);
}

#[test]
fn test_map_of_records_infer_jsonschema_wrapped() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--ndjson",
        "--map-threshold",
        "0",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__jsonschema_wrap", stdout_str);
}

#[test]
fn test_map_of_records_infer_avro_wrapped() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0", // force map detection
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__avro_wrap", avro_str);
}

// -- and again but with kv this time

#[test]
fn test_map_of_records_infer_jsonschema_kv() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--ndjson",
        "--map-threshold",
        "0",
        "--map-encoding",
        "kv",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__jsonschema_kv", stdout_str);
}

#[test]
fn test_map_of_records_infer_avro_kv() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0", // force map detection
        "--map-encoding",
        "kv",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__avro_kv", avro_str);
}

#[test]
fn test_map_of_records_infer_jsonschema_wrapped_kv() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
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
    assert_snapshot!("infer__jsonschema_wrap_kv", stdout_str);
}

#[test]
fn test_map_of_records_infer_avro_wrapped_kv() {
    let temp = write_ndjson(&sample_rows());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0", // force map detection
        "--map-encoding",
        "kv",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(output).unwrap();
    assert_snapshot!("infer__avro_wrap_kv", avro_str);
}
