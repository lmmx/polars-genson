// genson-cli/tests/integration.rs
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_valid_json() {
    let valid_json = r#"{"name": "Alice", "age": 30}"#;

    // Create a temporary file with valid JSON
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(valid_json.as_bytes())
        .expect("Failed to write to temp file");

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg(temp_file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"type\""))
        .stdout(predicate::str::contains("\"properties\""));
}

#[test]
fn test_invalid_json() {
    let invalid_json = r#"{"hello":"world}"#;
    let mut temp = NamedTempFile::new().unwrap();
    write!(temp, "{}", invalid_json).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg(temp.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid JSON input"))
        .stderr(predicate::str::contains("panicked").not())
        .stderr(predicate::str::contains("SIGABRT").not());
}

#[test]
fn test_malformed_json_variants() {
    let test_cases = vec![
        (r#"{"invalid": json}"#, "unquoted value"),
        (r#"{"incomplete":"#, "incomplete string"),
        (r#"{"trailing":,"#, "trailing comma"),
        (r#"{invalid: "json"}"#, "unquoted key"),
        (r#"{"nested": {"broken": json}}"#, "nested broken JSON"),
    ];

    for (invalid_json, description) in test_cases {
        println!("Testing: {}", description);

        // Create a temporary file with invalid JSON
        let mut temp_file = NamedTempFile::new()
            .unwrap_or_else(|_| panic!("Failed to create temp file for {}", description));
        temp_file
            .write_all(invalid_json.as_bytes())
            .unwrap_or_else(|_| panic!("Failed to write to temp file for {}", description));

        let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
        cmd.arg(temp_file.path());
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Invalid JSON input"))
            .stderr(predicate::str::contains("panicked").not())
            .stderr(predicate::str::contains("SIGABRT").not());
    }
}

#[test]
fn test_ndjson_format() {
    // Create valid NDJSON content - each line is a separate JSON object
    let ndjson_content = r#"{"name": "Alice", "age": 30}
{"name": "Bob", "age": 25, "city": "NYC"}
{"name": "Charlie", "score": 95.5}
"#;

    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(ndjson_content.as_bytes())
        .expect("Failed to write to temp file");

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg("--ndjson").arg(temp_file.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"type\""))
        .stdout(predicate::str::contains("\"properties\""))
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"age\""))
        .stderr(predicate::str::contains("Processed 1 JSON object(s)"));
}

#[test]
fn test_invalid_ndjson_format() {
    // NDJSON with one invalid line
    let invalid_ndjson = r#"{"name": "Alice", "age": 30}
{"invalid": json}
{"name": "Charlie", "score": 95.5}
"#;

    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(invalid_ndjson.as_bytes())
        .expect("Failed to write to temp file");

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg("--ndjson").arg(temp_file.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid JSON input"))
        .stderr(predicate::str::contains("panicked").not())
        .stderr(predicate::str::contains("SIGABRT").not());
}

#[test]
fn test_field_order_preservation() {
    let json_with_order = r#"{"z": "last", "b": "second", "a": "first"}"#;

    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(json_with_order.as_bytes())
        .expect("Failed to write to temp file");

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg(temp_file.path());
    let output = cmd.assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Find positions of properties in the schema output
    let z_pos = stdout.find("\"z\"").expect("Should find 'z' property");
    let b_pos = stdout.find("\"b\"").expect("Should find 'b' property");
    let a_pos = stdout.find("\"a\"").expect("Should find 'a' property");

    // Verify they appear in original order: z, b, a
    assert!(z_pos < b_pos, "Property 'z' should appear before 'b'");
    assert!(b_pos < a_pos, "Property 'b' should appear before 'a'");

    println!("✅ Field order preserved: z -> b -> a");
}

#[test]
fn test_nested_field_order_preservation() {
    let nested_json = r#"{"outer": {"z": 1, "a": 2}, "first": true}"#;

    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(nested_json.as_bytes())
        .expect("Failed to write to temp file");

    let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
    cmd.arg(temp_file.path());
    let output = cmd.assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    println!("Full output:\n{}", stdout);

    // Top level: "outer" should come before "first"
    let outer_pos = stdout
        .find("\"outer\":")
        .expect("Should find 'outer' property");
    let first_pos = stdout
        .find("\"first\":")
        .expect("Should find 'first' property");
    assert!(
        outer_pos < first_pos,
        "Property 'outer' should appear before 'first'"
    );

    // For nested properties, just find all occurrences and check their relative positions
    // Since the JSON is structured, we can find all occurrences and use context
    let z_positions: Vec<_> = stdout.match_indices("\"z\":").collect();
    let a_positions: Vec<_> = stdout.match_indices("\"a\":").collect();

    // Should find exactly one occurrence of each in the nested properties
    assert!(!z_positions.is_empty(), "Should find 'z' property");
    assert!(!a_positions.is_empty(), "Should find 'a' property");

    // The nested z should come before nested a
    // Look for the z that comes after "outer" in the JSON
    let nested_z_pos = z_positions
        .iter()
        .find(|(pos, _)| *pos > outer_pos)
        .expect("Should find nested 'z' after outer")
        .0;

    let nested_a_pos = a_positions
        .iter()
        .find(|(pos, _)| *pos > outer_pos)
        .expect("Should find nested 'a' after outer")
        .0;

    assert!(
        nested_z_pos < nested_a_pos,
        "Nested property 'z' should appear before 'a'"
    );

    println!(
        "✅ Nested field order preserved: z at {}, a at {}",
        nested_z_pos, nested_a_pos
    );
}
