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
        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

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

#[test]
fn test_field_order_preservation_core() {
    println!("=== Testing field order preservation in genson-core ===");

    let json_strings = vec![r#"{"z": "last", "b": "second", "a": "first"}"#.to_string()];

    let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

    match result {
        Ok(schema_result) => {
            let schema_str = serde_json::to_string_pretty(&schema_result.schema).unwrap();
            println!("Generated schema:\n{}", schema_str);

            // Find positions of properties in the schema
            let z_pos = schema_str.find("\"z\":").expect("Should find 'z' property");
            let b_pos = schema_str.find("\"b\":").expect("Should find 'b' property");
            let a_pos = schema_str.find("\"a\":").expect("Should find 'a' property");

            // Verify they appear in original order: z, b, a
            assert!(z_pos < b_pos, "Property 'z' should appear before 'b'");
            assert!(b_pos < a_pos, "Property 'b' should appear before 'a'");

            println!("✅ Field order preserved: z -> b -> a");
        }
        Err(error_msg) => {
            panic!("Schema inference failed: {}", error_msg);
        }
    }
}

#[test]
fn test_multiple_objects_field_order() {
    println!("=== Testing field order with multiple JSON objects ===");

    let json_strings = vec![
        r#"{"z": 1, "b": 2}"#.to_string(),
        r#"{"b": 3, "a": 4, "z": 5}"#.to_string(), // Different order, but z,b should win
        r#"{"c": 6, "z": 7, "b": 8}"#.to_string(), // New field 'c', should appear last
    ];

    let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

    match result {
        Ok(schema_result) => {
            let schema_str = serde_json::to_string_pretty(&schema_result.schema).unwrap();
            println!("Generated schema (multiple objects):\n{}", schema_str);

            // Field order should be based on first occurrence: z, b, then a, then c
            let z_pos = schema_str.find("\"z\":").expect("Should find 'z' property");
            let b_pos = schema_str.find("\"b\":").expect("Should find 'b' property");
            let a_pos = schema_str.find("\"a\":").expect("Should find 'a' property");
            let c_pos = schema_str.find("\"c\":").expect("Should find 'c' property");

            assert!(
                z_pos < b_pos,
                "Property 'z' should appear before 'b' (first object order)"
            );
            assert!(b_pos < a_pos, "Property 'b' should appear before 'a'");
            assert!(
                a_pos < c_pos,
                "Property 'a' should appear before 'c' (c appeared last)"
            );

            println!("✅ Multiple objects field order preserved based on first occurrence");
        }
        Err(error_msg) => {
            panic!("Schema inference failed: {}", error_msg);
        }
    }
}

#[test]
fn test_wrap_root_with_ndjson() {
    let ndjson_input = r#"
{"en":{"language":"en","value":"Hello"}}
{"fr":{"language":"fr","value":"Bonjour"}}
"#;

    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        wrap_root: Some("labels".to_string()),
        ..Default::default()
    };

    let json_strings = vec![ndjson_input.to_string()];
    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference with wrap_root on NDJSON should succeed");

    let schema_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Schema with wrap_root+NDJSON:\n{}", schema_str);

    assert_eq!(result.processed_count, 1); // NDJSON counts as 1 input string

    // Top-level schema should have `labels` as the required field
    assert_eq!(result.schema["type"], "object");
    assert_eq!(result.schema["required"], serde_json::json!(["labels"]));
    assert!(result.schema["properties"]["labels"].is_object());
}
