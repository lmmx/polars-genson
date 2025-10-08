// genson-core/src/tests/parquet.rs
use super::*;
use tempfile::NamedTempFile;

#[test]
fn test_write_and_read_roundtrip() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![
        r#"{"name": "Alice", "age": 30}"#.to_string(),
        r#"{"name": "Bob", "age": 25}"#.to_string(),
    ];

    // Write
    write_string_column(path, "json_data", test_strings.clone(), None).unwrap();

    // Read back
    let result = read_string_column(path, "json_data").unwrap();

    assert_eq!(result, test_strings);
}

#[test]
fn test_read_nonexistent_column() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec!["test".to_string()];
    write_string_column(path, "data", test_strings, None).unwrap();

    let result = read_string_column(path, "wrong_name");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_write_and_read_with_metadata() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![
        r#"{"name": "Alice"}"#.to_string(),
        r#"{"name": "Bob"}"#.to_string(),
    ];

    let mut metadata = HashMap::new();
    metadata.insert("test_key".to_string(), "test_value".to_string());
    metadata.insert("schema_version".to_string(), "1.0".to_string());

    // Write with metadata
    write_string_column(path, "data", test_strings.clone(), Some(metadata.clone())).unwrap();

    // Read data back
    let result_strings = read_string_column(path, "data").unwrap();
    assert_eq!(result_strings, test_strings);

    // Read metadata back
    let result_metadata = read_parquet_metadata(path).unwrap();
    assert_eq!(result_metadata.get("test_key"), Some(&"test_value".to_string()));
    assert_eq!(result_metadata.get("schema_version"), Some(&"1.0".to_string()));
}

#[test]
fn test_write_without_metadata_read_empty_metadata() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec!["test".to_string()];
    write_string_column(path, "data", test_strings, None).unwrap();

    let metadata = read_parquet_metadata(path).unwrap();
    assert!(metadata.is_empty());
}

#[test]
#[ignore]
fn test_large_string_array_automatic_selection() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // Create strings that exceed i32::MAX in total bytes
    let large_string = "a".repeat(1_000_000); // 1MB string
    let count = 3000; // 3GB total
    let test_strings: Vec<String> = (0..count).map(|_| large_string.clone()).collect();

    // Should automatically use LargeUtf8
    write_string_column(path, "large_data", test_strings.clone(), None).unwrap();

    // Read back and verify first few (reading all 3GB would be slow)
    let result = read_string_column(path, "large_data").unwrap();
    assert_eq!(result.len(), count);
    assert_eq!(result[0], test_strings[0]);
}

#[test]
fn test_metadata_with_complex_json_schema() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![r#"{"a":1}"#.to_string()];

    // Simulate storing a complex schema as metadata
    let complex_schema = r#"{"type":"object","properties":{"a":{"type":"integer"}}}"#;
    let mut metadata = HashMap::new();
    metadata.insert("genson_avro_schema".to_string(), complex_schema.to_string());
    metadata.insert("genson_normalise_config".to_string(), r#"{"empty_as_null":true,"coerce_string":false,"map_encoding":"kv","wrap_root":null}"#.to_string());

    write_string_column(path, "data", test_strings, Some(metadata.clone())).unwrap();

    let result_metadata = read_parquet_metadata(path).unwrap();
    assert_eq!(result_metadata.get("genson_avro_schema"), Some(&complex_schema.to_string()));

    // Verify we can parse it back
    let schema_json = result_metadata.get("genson_avro_schema").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(schema_json).unwrap();
    assert_eq!(parsed["type"], "object");
}
