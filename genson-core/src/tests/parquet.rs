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
    write_string_column(path, "json_data", test_strings.clone()).unwrap();

    // Read back
    let result = read_string_column(path, "json_data").unwrap();

    assert_eq!(result, test_strings);
}

#[test]
fn test_read_nonexistent_column() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec!["test".to_string()];
    write_string_column(path, "data", test_strings).unwrap();

    let result = read_string_column(path, "wrong_name");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}
