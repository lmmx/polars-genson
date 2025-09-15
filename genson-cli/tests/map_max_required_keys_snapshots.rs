// genson-cli/tests/map_max_required_keys_snapshots.rs

use assert_cmd::Command;
use insta::{assert_json_snapshot, with_settings};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper: parse NDJSON CLI output into Vec<Value> for clean JSON snapshots.
fn parse_ndjson(output: &str) -> Vec<Value> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("CLI should emit valid JSON"))
        .collect()
}

/// Check if the current output matches the verified/blessed version
fn is_output_approved<T: Serialize>(snapshot_name: &str, output: &T) -> bool {
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

            // Serialize current output to JSON string for comparison
            if let Ok(current_json) = serde_json::to_string_pretty(output) {
                // Parse both as Value to ensure consistent formatting
                if let (Ok(verified_val), Ok(current_val)) = (
                    serde_json::from_str::<Value>(verified_output.trim()),
                    serde_json::from_str::<Value>(&current_json),
                ) {
                    return verified_val == current_val;
                }
            }
        }
    }
    false
}

/// Attach the input data as metadata and snapshot the given value.
fn snapshot_with_input<T: Serialize>(name: &str, input_data: &str, value: T, args: Vec<String>) {
    // Parse each NDJSON line of input into proper JSON
    let input_json: Vec<Value> = input_data
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect();

    // Check if this output matches the blessed/verified version
    let approved = is_output_approved(name, &value);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args,
            "input": input_json
        })
    }, {
        assert_json_snapshot!(name, value);
    });
}

/// Run CLI and return schema (as JSON Value) and the args used.
fn get_schema(
    data: &str,
    threshold: usize,
    max_rk: Option<usize>,
    avro: bool,
) -> (Value, Vec<String>) {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "{}", data).unwrap();

    let threshold_str = threshold.to_string();
    let rk_str = max_rk.map(|rk| rk.to_string());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--ndjson", "--map-threshold", &threshold_str];
    if avro {
        args.insert(0, "--avro");
    }
    if let Some(ref rk_val) = rk_str {
        args.extend_from_slice(&["--map-max-rk", rk_val]);
    }
    args.push(temp.path().to_str().unwrap());

    // Save args for return (excluding temp file path)
    let args_owned: Vec<String> = args[..args.len() - 1]
        .iter()
        .map(|s| s.to_string())
        .collect();

    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8(output).unwrap();
    (serde_json::from_str(&output_str).unwrap(), args_owned)
}

/// Run CLI and return normalized data (as Vec of JSON Values) and the args used.
fn get_normalized(
    data: &str,
    threshold: usize,
    max_rk: Option<usize>,
) -> (Vec<Value>, Vec<String>) {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "{}", data).unwrap();

    let threshold_str = threshold.to_string();
    let rk_str = max_rk.map(|rk| rk.to_string());

    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--normalise", "--ndjson", "--map-threshold", &threshold_str];
    if let Some(ref rk_val) = rk_str {
        args.extend_from_slice(&["--map-max-rk", rk_val]);
    }
    args.push(temp.path().to_str().unwrap());

    // Save args for return (excluding temp file path)
    let args_owned: Vec<String> = args[..args.len() - 1]
        .iter()
        .map(|s| s.to_string())
        .collect();

    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8(output).unwrap();
    (parse_ndjson(&output_str), args_owned)
}

/// Helper: create temp file with test data and return both schema and normalized output with their args
fn test_map_max_rk(
    data: &str,
    threshold: usize,
    max_rk: Option<usize>,
) -> (Value, Vec<Value>, Vec<String>, Vec<String>) {
    let (schema, schema_args) = get_schema(data, threshold, max_rk, false);
    let (normalized, norm_args) = get_normalized(data, threshold, max_rk);
    (schema, normalized, schema_args, norm_args)
}

/// Helper: create temp file with test data and return Avro schema with args
fn test_map_max_rk_avro(
    data: &str,
    threshold: usize,
    max_rk: Option<usize>,
) -> (Value, Vec<String>) {
    get_schema(data, threshold, max_rk, true)
}

#[test]
fn test_map_max_rk_none_existing_behavior() {
    // Tests default behavior when map_max_required_keys is None.
    //
    // Expected outputs:
    // - JSON Schema: `structured` field becomes Map (additionalProperties) because it meets
    //   map_threshold=3 and has homogeneous string values. `below_threshold` stays Record
    //   (properties) because it has only 1 key < threshold.
    // - Avro Schema: `structured` becomes "type": "map", `below_threshold` becomes "type": "record"
    // - Normalized: Map fields preserve key-value structure, Record fields have fixed schema
    let data = r#"
{"structured": {"req1": "val1", "req2": "val2", "req3": "val3"}}
{"structured": {"req1": "val4", "req2": "val5", "req3": "val6"}}
{"below_threshold": {"only": "one"}}
{"below_threshold": {"only": "two"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 3, None);
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 3, None);

    // Snapshot schema: structured meets threshold and is homogeneous → Map
    // below_threshold doesn't meet threshold → Record
    snapshot_with_input("schema_max_rk_none", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_none", data, avro_schema, avro_args);

    // Snapshot normalized data showing Map vs Record behavior
    snapshot_with_input("normalized_max_rk_none", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_zero_strict_optional_only() {
    // Tests strictest setting where only objects with 0 required keys can become Maps.
    //
    // Expected outputs:
    // - JSON Schema: `fully_optional` becomes Map (additionalProperties) because all keys
    //   are optional (0 ≤ 0). `has_required` stays Record because 1 required key > 0.
    // - Avro Schema: `fully_optional` becomes "type": "map", `has_required` becomes "type": "record"
    // - Normalized: Only fully optional structures get Map treatment
    let data = r#"
{"fully_optional": {"sometimes": "here", "other": "maybe"}}
{"fully_optional": {"different": "keys"}}
{"has_required": {"always": "present", "sometimes": "here"}}
{"has_required": {"always": "present", "other": "value"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 2, Some(0));
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 2, Some(0));

    // Snapshot schema: fully_optional has 0 required keys → Map
    // has_required has 1 required key → Record (blocked by max_rk=0)
    snapshot_with_input("schema_max_rk_zero", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_zero", data, avro_schema, avro_args);

    // Snapshot normalized data showing strict Map detection
    snapshot_with_input("normalized_max_rk_zero", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_one_moderate_stability() {
    // Tests moderate setting allowing Maps with up to 1 required key.
    //
    // Expected outputs:
    // - JSON Schema: `one_required` becomes Map (additionalProperties) because 1 required
    //   key ≤ 1. `two_required` stays Record because 2 required keys > 1.
    // - Avro Schema: `one_required` becomes "type": "map", `two_required` becomes "type": "record"
    // - Normalized: Objects with moderate stability (1 required key) get Map treatment
    let data = r#"
{"one_required": {"common": "always", "varies": "sometimes"}}
{"one_required": {"common": "always", "other": "different"}}
{"two_required": {"stable1": "always", "stable2": "present", "varies": "sometimes"}}
{"two_required": {"stable1": "always", "stable2": "present", "other": "value"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 2, Some(1));
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 2, Some(1));

    // Snapshot schema: one_required has 1 required key → Map (allowed)
    // two_required has 2 required keys → Record (blocked by max_rk=1)
    snapshot_with_input("schema_max_rk_one", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_one", data, avro_schema, avro_args);

    // Snapshot normalized data showing moderate Map detection
    snapshot_with_input("normalized_max_rk_one", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_two_lenient_stability() {
    // Tests lenient setting allowing Maps with up to 2 required keys.
    //
    // Expected outputs:
    // - JSON Schema: `two_required` becomes Map (additionalProperties) because 2 required
    //   keys ≤ 2. `three_required` stays Record because 3 required keys > 2.
    // - Avro Schema: `two_required` becomes "type": "map", `three_required` becomes "type": "record"
    // - Normalized: Objects with higher stability (2 required keys) still get Map treatment
    let data = r#"
{"two_required": {"common1": "always", "common2": "present", "varies": "sometimes"}}
{"two_required": {"common1": "always", "common2": "present", "other": "value"}}
{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "varies": "sometimes"}}
{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "other": "value"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 3, Some(2));
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 3, Some(2));

    // Snapshot schema: two_required has 2 required keys → Map (allowed)
    // three_required has 3 required keys → Record (blocked by max_rk=2)
    snapshot_with_input("schema_max_rk_two", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_two", data, avro_schema, avro_args);

    // Snapshot normalized data showing lenient Map detection
    snapshot_with_input("normalized_max_rk_two", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_boundary_conditions() {
    // Tests exact threshold boundaries to verify gate logic.
    //
    // Expected outputs:
    // - JSON Schema: `over_rk_limit` stays Record (2 required > 1).
    //   `at_rk_limit` becomes Map (1 required ≤ 1).
    // - Avro Schema: Two "type": "record" and one "type": "map"
    // - Normalized: Only the object exactly at the required key limit gets Map treatment
    let data = r#"
{"at_rk_limit": {"req1": "always", "optional2": "sometimes"}}
{"at_rk_limit": {"req1": "always"}}
{"over_rk_limit": {"req1": "always", "req2": "present", "optional3": "sometimes"}}
{"over_rk_limit": {"req1": "always", "req2": "present"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 2, Some(1));
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 2, Some(1));

    // Snapshot schema showing boundary behavior:
    // at_rk_limit: 2 keys, 1 required → Map (1 ≤ 1)
    // over_rk_limit: 2 keys, 2 required → Record (2 > 1)
    snapshot_with_input("schema_max_rk_boundary", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_boundary", data, avro_schema, avro_args);

    // Snapshot normalized data showing boundary cases
    snapshot_with_input("normalized_max_rk_boundary", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_complex_nested() {
    // Tests nested objects with different required key counts.
    //
    // Expected outputs:
    // - user: 3 required keys (id, name, role) > 2 → Record (blocked by max_rk)
    // - config: 2 required keys (host, port) ≤ 2 → Map (allowed by max_rk)
    let data = r#"
{"user": {"id": "1", "name": "Alice", "role": "admin"}, "config": {"host": "localhost", "port": "8080", "debug": "true"}}
{"user": {"id": "2", "name": "Bob", "role": "user"}, "config": {"host": "prod.com", "port": "443"}}
{"user": {"id": "3", "name": "Charlie", "role": "user"}, "config": {"host": "test.com", "port": "3000", "env": "test"}}
"#;

    let (schema, normalized, schema_args, norm_args) = test_map_max_rk(data, 2, Some(2));
    let (avro_schema, avro_args) = test_map_max_rk_avro(data, 2, Some(2));

    // Snapshot schema:
    // user: 3 required keys > 2 → Record (blocked by max_rk limit)
    // config: 2 required keys ≤ 2 → Map (allowed by max_rk limit)
    snapshot_with_input("schema_max_rk_nested", data, schema, schema_args);
    snapshot_with_input("avro_schema_max_rk_nested", data, avro_schema, avro_args);

    // Snapshot normalized data showing user as Record, config as Map
    snapshot_with_input("normalized_max_rk_nested", data, normalized, norm_args);
}

#[test]
fn test_map_max_rk_progression() {
    // Tests same dataset with different max_rk values to show progressive behavior.
    // Data has 2 required keys (always1, always2).
    //
    // Expected outputs:
    // - max_rk=0: `data` stays Record because 2 required > 0
    // - max_rk=1: `data` stays Record because 2 required > 1
    // - max_rk=2: `data` becomes Map because 2 required ≤ 2
    //
    // Avro schemas should show "type": "record" for rk0/rk1, "type": "map" for rk2
    // Normalized data should reflect the Record vs Map structure accordingly
    let data = r#"
{"data": {"always1": "val1", "always2": "val2", "sometimes": "val3"}}
{"data": {"always1": "val4", "always2": "val5"}}
{"data": {"always1": "val6", "always2": "val7", "other": "val8"}}
"#;

    // Test with max_rk=0: should be Record (2 required > 0)
    let (schema0, norm0, schema_args0, norm_args0) = test_map_max_rk(data, 2, Some(0));
    let (avro0, avro_args0) = test_map_max_rk_avro(data, 2, Some(0));

    // Test with max_rk=1: should be Record (2 required > 1)
    let (schema1, norm1, schema_args1, norm_args1) = test_map_max_rk(data, 2, Some(1));
    let (avro1, avro_args1) = test_map_max_rk_avro(data, 2, Some(1));

    // Test with max_rk=2: should be Map (2 required ≤ 2)
    let (schema2, norm2, schema_args2, norm_args2) = test_map_max_rk(data, 2, Some(2));
    let (avro2, avro_args2) = test_map_max_rk_avro(data, 2, Some(2));

    // Snapshot all three to show progression
    snapshot_with_input("schema_progression_rk0", data, schema0, schema_args0);
    snapshot_with_input("avro_progression_rk0", data, avro0, avro_args0);
    snapshot_with_input("normalized_progression_rk0", data, norm0, norm_args0);

    snapshot_with_input("schema_progression_rk1", data, schema1, schema_args1);
    snapshot_with_input("avro_progression_rk1", data, avro1, avro_args1);
    snapshot_with_input("normalized_progression_rk1", data, norm1, norm_args1);

    snapshot_with_input("schema_progression_rk2", data, schema2, schema_args2);
    snapshot_with_input("avro_progression_rk2", data, avro2, avro_args2);
    snapshot_with_input("normalized_progression_rk2", data, norm2, norm_args2);
}
