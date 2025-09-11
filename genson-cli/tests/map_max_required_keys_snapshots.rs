// genson-cli/tests/map_max_required_keys_snapshots.rs

use assert_cmd::Command;
use insta::{assert_json_snapshot, with_settings};
use serde::Serialize;
use serde_json::Value;
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

/// Run CLI and return schema (as JSON Value).
fn get_schema(data: &str, threshold: usize, max_rk: Option<usize>, avro: bool) -> Value {
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
    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8(output).unwrap();
    serde_json::from_str(&output_str).unwrap()
}

/// Run CLI and return normalized data (as Vec of JSON Values).
fn get_normalized(data: &str, threshold: usize, max_rk: Option<usize>) -> Vec<Value> {
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
    cmd.args(&args);

    let output = cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8(output).unwrap();
    parse_ndjson(&output_str)
}

/// Attach the input data as metadata and snapshot the given value.
fn snapshot_with_input<T: Serialize>(name: &str, input_data: &str, value: T) {
    // Parse each NDJSON line of input into proper JSON
    let input_json: Vec<Value> = input_data
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect();

    with_settings!({
        info => &serde_json::json!({ "input": input_json })
    }, {
        assert_json_snapshot!(name, value);
    });
}

/// Helper: create temp file with test data and return both schema and normalized output
fn test_map_max_rk(data: &str, threshold: usize, max_rk: Option<usize>) -> (Value, Vec<Value>) {
    (
        get_schema(data, threshold, max_rk, false),
        get_normalized(data, threshold, max_rk),
    )
}

/// Helper: create temp file with test data and return Avro schema
fn test_map_max_rk_avro(data: &str, threshold: usize, max_rk: Option<usize>) -> Value {
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

    let (schema, normalized) = test_map_max_rk(data, 3, None);
    let avro_schema = test_map_max_rk_avro(data, 3, None);

    // Snapshot schema: structured meets threshold and is homogeneous → Map
    // below_threshold doesn't meet threshold → Record
    snapshot_with_input("schema_max_rk_none", data, schema);
    snapshot_with_input("avro_schema_max_rk_none", data, avro_schema);

    // Snapshot normalized data showing Map vs Record behavior
    snapshot_with_input("normalized_max_rk_none", data, normalized);
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

    let (schema, normalized) = test_map_max_rk(data, 2, Some(0));
    let avro_schema = test_map_max_rk_avro(data, 2, Some(0));

    // Snapshot schema: fully_optional has 0 required keys → Map
    // has_required has 1 required key → Record (blocked by max_rk=0)
    snapshot_with_input("schema_max_rk_zero", data, schema);
    snapshot_with_input("avro_schema_max_rk_zero", data, avro_schema);

    // Snapshot normalized data showing strict Map detection
    snapshot_with_input("normalized_max_rk_zero", data, normalized);
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

    let (schema, normalized) = test_map_max_rk(data, 2, Some(1));
    let avro_schema = test_map_max_rk_avro(data, 2, Some(1));

    // Snapshot schema: one_required has 1 required key → Map (allowed)
    // two_required has 2 required keys → Record (blocked by max_rk=1)
    snapshot_with_input("schema_max_rk_one", data, schema);
    snapshot_with_input("avro_schema_max_rk_one", data, avro_schema);

    // Snapshot normalized data showing moderate Map detection
    snapshot_with_input("normalized_max_rk_one", data, normalized);
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

    let (schema, normalized) = test_map_max_rk(data, 3, Some(2));
    let avro_schema = test_map_max_rk_avro(data, 3, Some(2));

    // Snapshot schema: two_required has 2 required keys → Map (allowed)
    // three_required has 3 required keys → Record (blocked by max_rk=2)
    snapshot_with_input("schema_max_rk_two", data, schema);
    snapshot_with_input("avro_schema_max_rk_two", data, avro_schema);

    // Snapshot normalized data showing lenient Map detection
    snapshot_with_input("normalized_max_rk_two", data, normalized);
}

#[test]
fn test_map_max_rk_boundary_conditions() {
    // Tests exact threshold boundaries to verify gate logic.
    //
    // Expected outputs:
    // - JSON Schema: `at_map_threshold` and `over_rk_limit` stay Records (2 required > 1).
    //   `at_rk_limit` becomes Map (1 required ≤ 1).
    // - Avro Schema: Two "type": "record" and one "type": "map"
    // - Normalized: Only the object exactly at the required key limit gets Map treatment
    let data = r#"
{"at_map_threshold": {"key1": "val1", "key2": "val2"}}
{"at_map_threshold": {"key1": "val3", "key2": "val4"}}
{"at_rk_limit": {"required": "always", "optional": "sometimes"}}
{"at_rk_limit": {"required": "always"}}
{"over_rk_limit": {"req1": "always", "req2": "present", "optional": "sometimes"}}
{"over_rk_limit": {"req1": "always", "req2": "present"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(1));
    let avro_schema = test_map_max_rk_avro(data, 2, Some(1));

    // Snapshot schema showing boundary behavior:
    // at_map_threshold: 2 keys, 2 required → Record (2 > 1)
    // at_rk_limit: 2 keys, 1 required → Map (1 ≤ 1)
    // over_rk_limit: 2 keys, 2 required → Record (2 > 1)
    snapshot_with_input("schema_max_rk_boundary", data, schema);
    snapshot_with_input("avro_schema_max_rk_boundary", data, avro_schema);

    // Snapshot normalized data showing boundary cases
    snapshot_with_input("normalized_max_rk_boundary", data, normalized);
}

#[test]
fn test_map_max_rk_complex_nested() {
    // Tests nested objects with different required key patterns and homogeneity requirements.
    //
    // Expected outputs:
    // - JSON Schema: Root level stays Record (user+config required, but mixed types).
    //   `user` stays Record (id+name required, but mixed int/string types).
    //   `config` becomes Map (host+port required ≤ 2, homogeneous strings).
    // - Avro Schema: Root and user are "type": "record", config is "type": "map"
    // - Normalized: Only config field gets Map treatment due to homogeneity + required key count
    let data = r#"
{"user": {"id": 1, "name": "Alice"}, "config": {"host": "localhost", "port": "8080", "debug": "true"}}
{"user": {"id": 2, "name": "Bob"}, "config": {"host": "prod.com", "port": "443"}}
{"user": {"id": 3, "name": "Charlie"}, "config": {"host": "test.com", "port": "3000", "env": "test"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(2));
    let avro_schema = test_map_max_rk_avro(data, 2, Some(2));

    // Snapshot schema:
    // Root: user, config both required (2 ≤ 2) → could be Map but fails homogeneity
    // user: id, name both required (2 ≤ 2) → could be Map but fails homogeneity
    // config: host, port required, others optional (2 ≤ 2) → Map (homogeneous strings)
    snapshot_with_input("schema_max_rk_nested", data, schema);
    snapshot_with_input("avro_schema_max_rk_nested", data, avro_schema);

    // Snapshot normalized data showing nested Map/Record decisions
    snapshot_with_input("normalized_max_rk_nested", data, normalized);
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
    let (schema0, norm0) = test_map_max_rk(data, 2, Some(0));
    let avro0 = test_map_max_rk_avro(data, 2, Some(0));

    // Test with max_rk=1: should be Record (2 required > 1)
    let (schema1, norm1) = test_map_max_rk(data, 2, Some(1));
    let avro1 = test_map_max_rk_avro(data, 2, Some(1));

    // Test with max_rk=2: should be Map (2 required ≤ 2)
    let (schema2, norm2) = test_map_max_rk(data, 2, Some(2));
    let avro2 = test_map_max_rk_avro(data, 2, Some(2));

    // Snapshot all three to show progression
    snapshot_with_input("schema_progression_rk0", data, schema0);
    snapshot_with_input("avro_progression_rk0", data, avro0);
    snapshot_with_input("normalized_progression_rk0", data, norm0);

    snapshot_with_input("schema_progression_rk1", data, schema1);
    snapshot_with_input("avro_progression_rk1", data, avro1);
    snapshot_with_input("normalized_progression_rk1", data, norm1);

    snapshot_with_input("schema_progression_rk2", data, schema2);
    snapshot_with_input("avro_progression_rk2", data, avro2);
    snapshot_with_input("normalized_progression_rk2", data, norm2);
}
