// genson-cli/tests/integration.rs
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_valid_json() {
    let valid_json = r#"{"name": "Alice", "age": 30}"#;
    
    // Create a temporary file with valid JSON
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file.write_all(valid_json.as_bytes()).expect("Failed to write to temp file");
    
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
            .expect(&format!("Failed to create temp file for {}", description));
        temp_file.write_all(invalid_json.as_bytes())
            .expect(&format!("Failed to write to temp file for {}", description));
        
        let mut cmd = assert_cmd::Command::cargo_bin("genson-cli").unwrap();
        cmd.arg(temp_file.path());
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Invalid JSON input"))
            .stderr(predicate::str::contains("panicked").not())
            .stderr(predicate::str::contains("SIGABRT").not());
    }
}