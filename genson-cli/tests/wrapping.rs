use assert_cmd::Command;
use insta::{assert_json_snapshot, assert_snapshot, with_settings};
use serde_json::Value;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

// Note: pretty sure there's a lot of redundant test cases here, tidy up later (but these work)

/// Parse NDJSON CLI output into Vec<Value> for pretty JSON snapshots.
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

/// Check if the current output matches the verified/blessed version for string snapshots
fn is_output_approved(snapshot_name: &str, output: &str) -> bool {
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

/// Check if the current output matches the verified/blessed version for JSON snapshots
fn is_json_output_approved(snapshot_name: &str, output: &[Value]) -> bool {
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
                                                                       // Parse the verified JSON and compare with current output
            if let Ok(verified_json) = serde_json::from_str::<Vec<Value>>(verified_output.trim()) {
                return verified_json == *output;
            }
        }
    }
    false
}

/// Run genson-cli with given args, writing `rows` into a temp NDJSON file,
/// and snapshot the CLI output with the input data included in the header.
fn run_snapshot(name: &str, rows: &[&str], args: &[&str], json_snapshot: bool, pretty: bool) {
    let mut temp = NamedTempFile::new().unwrap();
    for row in rows {
        writeln!(temp, "{}", row).unwrap();
    }

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();

    // Build all args into a Vec<&str>
    let mut all_args: Vec<&str> = args.to_vec();
    all_args.push(temp.path().to_str().unwrap());

    let args_for_metadata = all_args.clone();
    cmd.args(&all_args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout_str = String::from_utf8(output).unwrap();

    let value = if pretty {
        pretty_print_ndjson(&stdout_str)
    } else if json_snapshot {
        serde_json::to_string_pretty(&parse_ndjson(&stdout_str)).unwrap()
    } else {
        stdout_str.clone()
    };

    let input_json: Vec<Value> = rows
        .iter()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect();

    // Check approval based on snapshot type
    let approved = if json_snapshot {
        let parsed_output = parse_ndjson(&stdout_str);
        is_json_output_approved(name, &parsed_output)
    } else {
        is_output_approved(name, &value)
    };

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args_for_metadata[..args_for_metadata.len()-1],
            "input": input_json
        })
    }, {
        if json_snapshot {
            assert_json_snapshot!(name, parse_ndjson(&stdout_str));
        } else {
            assert_snapshot!(name, value);
        }
    });
}

#[test]
fn test_wrap_root_snapshot() {
    let rows = [
        r#"{"en": {"language":"en","value":"Hello"}, "fr": {"language":"fr","value":"Bonjour"}, "es": {"language": "es","value":"Hola"}}"#,
    ];
    run_snapshot(
        "wrap_root__cli_normalise",
        &rows,
        &[
            "--normalise",
            "--ndjson",
            "--map-threshold",
            "3",
            "--map-encoding",
            "kv",
            "--wrap-root",
            "labels",
        ],
        true,  // json_snapshot
        false, // pretty
    );
}

#[test]
fn test_normalise_labels_wrap_map_of_structs_snapshot() {
    let rows = [
        r#"{"en":{"language":"en","value":"Jack Bauer"},"fr":{"language":"fr","value":"Jack Bauer"}}"#,
        r#"{"en":{"language":"en","value":"happiness"},"fr":{"language":"fr","value":"bonheur"},"rn":{"language":"rn","value":"Umunezero"}}"#,
    ];

    // Normalise run
    run_snapshot(
        "labels_map_of_structs__normalise",
        &rows,
        &[
            "--normalise",
            "--ndjson",
            "--map-threshold",
            "3",
            "--map-encoding",
            "kv",
            "--wrap-root",
            "labels",
        ],
        false,
        true, // pretty
    );

    // Avro schema run
    run_snapshot(
        "labels_map_of_structs__avro",
        &rows,
        &[
            "--avro",
            "--ndjson",
            "--map-threshold",
            "3",
            "--map-encoding",
            "kv",
            "--wrap-root",
            "labels",
        ],
        false,
        false,
    );
}

#[test]
fn test_avro_map_of_record_values_snapshot() {
    let rows = [
        r#"{"en":{"language":"en","value":"Hello"},"fr":{"language":"fr","value":"Bonjour"}}"#,
        r#"{"en":{"language":"en","value":"Hi"},"fr":{"language":"fr","value":"Salut"},"rn":{"language":"rn","value":"Umunezero"}}"#,
    ];

    // Root-level Avro schema
    run_snapshot(
        "map_of_record_values__avro_root",
        &rows,
        &["--avro", "--ndjson", "--map-threshold", "3"],
        false,
        false,
    );

    // Wrapped Avro schema
    run_snapshot(
        "map_of_record_values__avro_wrapped",
        &rows,
        &[
            "--avro",
            "--ndjson",
            "--map-threshold",
            "3",
            "--wrap-root",
            "labels",
        ],
        false,
        false,
    );
}
