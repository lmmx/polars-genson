use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};

#[test]
fn test_invalid_json_integration() {
    println!("=== Testing invalid JSON that crashes genson-rs ===");

    let test_cases = vec![
        (r#"{"invalid": json}"#, "unquoted value"),
        (r#"{"hello":"world}"#, "missing closing quote"),
        (r#"{"incomplete":"#, "incomplete string"),
        (r#"{"trailing":,"#, "trailing comma"),
        (r#"{invalid: "json"}"#, "unquoted key"),
        (r#"{"nested": {"broken": json}}"#, "nested broken JSON"),
    ];

    for (invalid_json, description) in test_cases {
        println!("\n--- Testing: {} ---", description);
        println!("Input: {}", invalid_json);

        let json_strings = vec![invalid_json.to_string()];

        // This should NOT panic - it should return a proper error
        let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

        match result {
            Ok(schema_result) => {
                panic!(
                    "Expected error for invalid JSON '{}' but got success: {:?}",
                    invalid_json, schema_result
                );
            }
            Err(error_msg) => {
                println!("✅ Got expected error: {}", error_msg);
                // Verify it's a proper error message, not a panic message
                assert!(
                    !error_msg.contains("panicked"),
                    "Error message should not contain 'panicked': {}",
                    error_msg
                );
                assert!(!error_msg.is_empty(), "Error message should not be empty");
            }
        }
    }

    println!("\n=== All invalid JSON cases handled properly ===");
}

#[test]
fn test_mixed_valid_and_invalid_json() {
    println!("=== Testing mixed valid and invalid JSON ===");

    let json_strings = vec![
        r#"{"name": "Alice", "age": 30}"#.to_string(), // Valid
        r#"{"invalid": json}"#.to_string(),            // Invalid - should cause problems
        r#"{"name": "Bob", "age": 25}"#.to_string(),   // Valid
    ];

    // This should handle the invalid JSON gracefully
    let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

    // Should either:
    // 1. Return an error (preferred)
    // 2. Skip the invalid JSON and process the valid ones
    match result {
        Ok(schema_result) => {
            println!("✅ Processed with some success: {:?}", schema_result);
            // If it succeeds, it should have processed the valid JSON
            assert!(
                schema_result.processed_count > 0,
                "Should have processed at least some JSON"
            );
        }
        Err(error_msg) => {
            println!("✅ Got expected error for mixed input: {}", error_msg);
            // Should be a clean error, not a panic
            assert!(
                !error_msg.contains("panicked"),
                "Error should not contain 'panicked': {}",
                error_msg
            );
        }
    }
}

#[test]
fn test_only_invalid_json() {
    println!("=== Testing only invalid JSON ===");

    let json_strings = vec![
        r#"{"invalid": json}"#.to_string(),
        r#"{"also": invalid}"#.to_string(),
    ];

    let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

    // Should definitely return an error
    assert!(result.is_err(), "Should return error for all invalid JSON");

    let error_msg = result.unwrap_err();
    println!("✅ Got expected error: {}", error_msg);

    // Should be a clean error message
    assert!(
        !error_msg.contains("panicked"),
        "Error should not contain 'panicked': {}",
        error_msg
    );
    assert!(!error_msg.is_empty(), "Error message should not be empty");
}
