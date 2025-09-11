// genson-cli/tests/map_max_required_keys_snapshots.rs

use assert_cmd::Command;
use insta::assert_json_snapshot;
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

/// Helper: create temp file with test data and return both schema and normalized output
fn test_map_max_rk(data: &str, threshold: usize, max_rk: Option<usize>) -> (Value, Vec<Value>) {
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "{}", data).unwrap();

    let threshold_str = threshold.to_string();
    let rk_str = max_rk.map(|rk| rk.to_string());

    // Get schema
    let mut schema_cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec!["--ndjson", "--map-threshold", &threshold_str];
    if let Some(ref rk_val) = rk_str {
        args.extend_from_slice(&["--map-max-rk", rk_val]);
    }
    args.push(temp.path().to_str().unwrap());
    schema_cmd.args(&args);

    let schema_output = schema_cmd.assert().success().get_output().stdout.clone();
    let schema_str = String::from_utf8(schema_output).unwrap();
    let schema: Value = serde_json::from_str(&schema_str).unwrap();

    // Get normalized output
    let mut norm_cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut norm_args = vec!["--normalise", "--ndjson", "--map-threshold", &threshold_str];
    if let Some(ref rk_val) = rk_str {
        norm_args.extend_from_slice(&["--map-max-rk", rk_val]);
    }
    norm_args.push(temp.path().to_str().unwrap());
    norm_cmd.args(&norm_args);

    let norm_output = norm_cmd.assert().success().get_output().stdout.clone();
    let norm_str = String::from_utf8(norm_output).unwrap();
    let normalized = parse_ndjson(&norm_str);

    (schema, normalized)
}

#[test]
fn test_map_max_rk_none_existing_behavior() {
    // Test data: structured objects with varying required key counts
    let data = r#"
{"structured": {"req1": "val1", "req2": "val2", "req3": "val3"}}
{"structured": {"req1": "val4", "req2": "val5", "req3": "val6"}}
{"below_threshold": {"only": "one"}}
{"below_threshold": {"only": "two"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 3, None);

    // Snapshot schema: structured meets threshold and is homogeneous → Map
    // below_threshold doesn't meet threshold → Record
    assert_json_snapshot!("schema_max_rk_none", schema);

    // Snapshot normalized data showing Map vs Record behavior
    assert_json_snapshot!("normalized_max_rk_none", normalized);
}

#[test]
fn test_map_max_rk_zero_strict_optional_only() {
    // Test data: objects with 0 vs 1+ required keys
    let data = r#"
{"fully_optional": {"sometimes": "here", "other": "maybe"}}
{"fully_optional": {"different": "keys"}}
{"has_required": {"always": "present", "sometimes": "here"}}
{"has_required": {"always": "present", "other": "value"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(0));

    // Snapshot schema: fully_optional has 0 required keys → Map
    // has_required has 1 required key → Record (blocked by max_rk=0)
    assert_json_snapshot!("schema_max_rk_zero", schema);

    // Snapshot normalized data showing strict Map detection
    assert_json_snapshot!("normalized_max_rk_zero", normalized);
}

#[test]
fn test_map_max_rk_one_moderate_stability() {
    // Test data: objects with 1 vs 2+ required keys
    let data = r#"
{"one_required": {"common": "always", "varies": "sometimes"}}
{"one_required": {"common": "always", "other": "different"}}
{"two_required": {"stable1": "always", "stable2": "present", "varies": "sometimes"}}
{"two_required": {"stable1": "always", "stable2": "present", "other": "value"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(1));

    // Snapshot schema: one_required has 1 required key → Map (allowed)
    // two_required has 2 required keys → Record (blocked by max_rk=1)
    assert_json_snapshot!("schema_max_rk_one", schema);

    // Snapshot normalized data showing moderate Map detection
    assert_json_snapshot!("normalized_max_rk_one", normalized);
}

#[test]
fn test_map_max_rk_two_lenient_stability() {
    // Test data: objects with 2 vs 3+ required keys
    let data = r#"
{"two_required": {"common1": "always", "common2": "present", "varies": "sometimes"}}
{"two_required": {"common1": "always", "common2": "present", "other": "value"}}
{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "varies": "sometimes"}}
{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "other": "value"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 3, Some(2));

    // Snapshot schema: two_required has 2 required keys → Map (allowed)
    // three_required has 3 required keys → Record (blocked by max_rk=2)
    assert_json_snapshot!("schema_max_rk_two", schema);

    // Snapshot normalized data showing lenient Map detection
    assert_json_snapshot!("normalized_max_rk_two", normalized);
}

#[test]
fn test_map_max_rk_boundary_conditions() {
    // Test edge cases: exactly at thresholds
    let data = r#"
{"at_map_threshold": {"key1": "val1", "key2": "val2"}}
{"at_map_threshold": {"key1": "val3", "key2": "val4"}}
{"at_rk_limit": {"required": "always", "optional": "sometimes"}}
{"at_rk_limit": {"required": "always"}}
{"over_rk_limit": {"req1": "always", "req2": "present", "optional": "sometimes"}}
{"over_rk_limit": {"req1": "always", "req2": "present"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(1));

    // Snapshot schema showing boundary behavior:
    // at_map_threshold: 2 keys, 2 required → Record (2 > 1)
    // at_rk_limit: 2 keys, 1 required → Map (1 ≤ 1)
    // over_rk_limit: 2 keys, 2 required → Record (2 > 1)
    assert_json_snapshot!("schema_max_rk_boundary", schema);

    // Snapshot normalized data showing boundary cases
    assert_json_snapshot!("normalized_max_rk_boundary", normalized);
}

#[test]
fn test_map_max_rk_complex_nested() {
    // Test nested objects with different required key patterns
    let data = r#"
{"user": {"id": 1, "name": "Alice"}, "config": {"host": "localhost", "port": "8080", "debug": "true"}}
{"user": {"id": 2, "name": "Bob"}, "config": {"host": "prod.com", "port": "443"}}
{"user": {"id": 3, "name": "Charlie"}, "config": {"host": "test.com", "port": "3000", "env": "test"}}
"#;

    let (schema, normalized) = test_map_max_rk(data, 2, Some(2));

    // Snapshot schema:
    // Root: user, config both required (2 ≤ 2) → could be Map but fails homogeneity
    // user: id, name both required (2 ≤ 2) → could be Map but fails homogeneity
    // config: host, port required, others optional (2 ≤ 2) → Map (homogeneous strings)
    assert_json_snapshot!("schema_max_rk_nested", schema);

    // Snapshot normalized data showing nested Map/Record decisions
    assert_json_snapshot!("normalized_max_rk_nested", normalized);
}

#[test]
fn test_map_max_rk_progression() {
    // Single dataset tested with different max_rk values to show progression
    let data = r#"
{"data": {"always1": "val1", "always2": "val2", "sometimes": "val3"}}
{"data": {"always1": "val4", "always2": "val5"}}
{"data": {"always1": "val6", "always2": "val7", "other": "val8"}}
"#;

    // Test with max_rk=0: should be Record (2 required > 0)
    let (schema0, norm0) = test_map_max_rk(data, 2, Some(0));

    // Test with max_rk=1: should be Record (2 required > 1)
    let (schema1, norm1) = test_map_max_rk(data, 2, Some(1));

    // Test with max_rk=2: should be Map (2 required ≤ 2)
    let (schema2, norm2) = test_map_max_rk(data, 2, Some(2));

    // Snapshot all three to show progression
    assert_json_snapshot!("schema_progression_rk0", schema0);
    assert_json_snapshot!("normalized_progression_rk0", norm0);

    assert_json_snapshot!("schema_progression_rk1", schema1);
    assert_json_snapshot!("normalized_progression_rk1", norm1);

    assert_json_snapshot!("schema_progression_rk2", schema2);
    assert_json_snapshot!("normalized_progression_rk2", norm2);
}
