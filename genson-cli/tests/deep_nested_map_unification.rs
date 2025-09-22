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

/// Run genson-cli with deep nested unification settings  
fn run_genson_deep_nested(name: &str, rows: Vec<&str>, extra_args: &[&str]) {
    let temp = write_ndjson(&rows);

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec![
        "--ndjson",
        "--map-threshold",
        "0",
        "--unify-maps",
        "--wrap-root",
        "places",
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

/// Deep nested structure with int vs string leaf conflict
/// Structure: array -> map -> array -> map -> array -> map -> int/string
fn deep_nested_conflicting_rows() -> Vec<&'static str> {
    vec![
        r#"{"home":[{"rooms":[{"kitchen":[{"temp":{"celsius":22}}]}]}]}"#,
        r#"{"work":[{"rooms":[{"office":[{"desk":{"type":"standing"}}]}]}]}"#,
    ]
}

#[test]
fn test_deep_nested_conflicting_jsonschema() {
    run_genson_deep_nested(
        "deep_nested_conflicting__jsonschema",
        deep_nested_conflicting_rows(),
        &[],
    );
}

#[test]
fn test_deep_nested_conflicting_avro() {
    run_genson_deep_nested(
        "deep_nested_conflicting__avro",
        deep_nested_conflicting_rows(),
        &["--avro"],
    );
}

#[test]
fn test_deep_nested_conflicting_normalize() {
    run_genson_deep_nested(
        "deep_nested_conflicting__normalize",
        deep_nested_conflicting_rows(),
        &["--normalise"],
    );
}
