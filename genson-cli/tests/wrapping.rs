use assert_cmd::Command;
use insta::{assert_json_snapshot, assert_snapshot};
use serde_json::Value;
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

#[test]
fn test_wrap_root_snapshot() {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(
        temp,
        r#"{{"en": {{"language":"en","value":"Hello"}}, "fr": {{"language":"fr","value":"Bonjour"}}}}"#
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
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

    assert_json_snapshot!("wrap_root__cli_normalise", parse_ndjson(&stdout_str));
}

#[test]
fn test_normalise_labels_wrap_map_of_structs_snapshot() {
    // NDJSON where each row is literally a raw JSON string
    let mut temp = NamedTempFile::new().unwrap();

    // First row: two languages
    writeln!(
        temp,
        r#"{{"en":{{"language":"en","value":"Jack Bauer"}},"fr":{{"language":"fr","value":"Jack Bauer"}}}}"#
    )
    .unwrap();

    // Second row: three languages
    writeln!(
        temp,
        r#"{{"en":{{"language":"en","value":"happiness"}},"fr":{{"language":"fr","value":"bonheur"}},"rn":{{"language":"rn","value":"Umunezero"}}}}"#
    )
    .unwrap();

    // Run CLI with normalisation enabled and force map detection
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--normalise",
        "--ndjson",
        "--map-threshold",
        "0", // ensure it's treated as a map
        "--map-encoding",
        "kv",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    let pretty_output = pretty_print_ndjson(&stdout_str);

    assert_snapshot!("labels_map_of_structs__normalise", pretty_output);

    // ---------- Second call: avro schema ----------
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-encoding",
        "kv",
        "--map-threshold",
        "0",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let avro_output = cmd.assert().success().get_output().stdout.clone();
    let avro_str = String::from_utf8(avro_output).unwrap();

    assert_snapshot!("labels_map_of_structs__avro", avro_str);
}

#[test]
fn test_avro_map_of_record_values_snapshot() {
    // NDJSON where each row is a map<string, record{language, value}>
    let mut temp = NamedTempFile::new().unwrap();

    // Row 1
    writeln!(
        temp,
        r#"{{"en":{{"language":"en","value":"Hello"}},"fr":{{"language":"fr","value":"Bonjour"}}}}"#
    )
    .unwrap();

    // Row 2
    writeln!(
        temp,
        r#"{{"en":{{"language":"en","value":"Hi"}},"fr":{{"language":"fr","value":"Salut"}},"rn":{{"language":"rn","value":"Umunezero"}}}}"#
    )
    .unwrap();

    // ------------ Call 1: Avro schema at root ------------
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0", // force map detection
        temp.path().to_str().unwrap(),
    ]);

    let out_root = cmd.assert().success().get_output().stdout.clone();
    let avro_root_str = String::from_utf8(out_root).unwrap();
    assert_snapshot!("map_of_record_values__avro_root", avro_root_str);

    // ------------ Call 2: Avro schema with wrap-root=labels ------------
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args([
        "--avro",
        "--ndjson",
        "--map-threshold",
        "0",
        "--wrap-root",
        "labels",
        temp.path().to_str().unwrap(),
    ]);

    let out_wrapped = cmd.assert().success().get_output().stdout.clone();
    let avro_wrapped_str = String::from_utf8(out_wrapped).unwrap();
    assert_snapshot!("map_of_record_values__avro_wrapped", avro_wrapped_str);
}
