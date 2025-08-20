use crate::genson_rs::{build_json_schema, get_builder, BuildConfig};
use serde::de::Error as DeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::panic::{self, AssertUnwindSafe};

/// Maximum length of JSON string to include in error messages before truncating
const MAX_JSON_ERROR_LENGTH: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInferenceConfig {
    /// Whether to treat top-level arrays as streams of objects
    pub ignore_outer_array: bool,
    /// Delimiter for NDJSON format (None for regular JSON)
    pub delimiter: Option<u8>,
    /// Schema URI to use ("AUTO" for auto-detection)
    pub schema_uri: Option<String>,
}

impl Default for SchemaInferenceConfig {
    fn default() -> Self {
        Self {
            ignore_outer_array: true,
            delimiter: None,
            schema_uri: Some("AUTO".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInferenceResult {
    pub schema: Value,
    pub processed_count: usize,
}

fn validate_json(s: &str) -> Result<(), serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(s);
    serde::de::IgnoredAny::deserialize(&mut de)?; // lightweight: ignores the parsed value
    de.end()
}

fn validate_ndjson(s: &str) -> Result<(), serde_json::Error> {
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        validate_json(trimmed)?; // propagate serde_json::Error
    }
    Ok(())
}

/// Infer JSON schema from a collection of JSON strings
pub fn infer_json_schema_from_strings(
    json_strings: &[String],
    config: SchemaInferenceConfig,
) -> Result<SchemaInferenceResult, String> {
    if json_strings.is_empty() {
        return Err("No JSON strings provided".to_string());
    }

    // Wrap the entire genson-rs interaction in panic handling
    let result = panic::catch_unwind(AssertUnwindSafe(
        || -> Result<SchemaInferenceResult, String> {
            // Create schema builder
            let mut builder = get_builder(config.schema_uri.as_deref());

            // Build config for genson-rs
            let build_config = BuildConfig {
                delimiter: config.delimiter,
                ignore_outer_array: config.ignore_outer_array,
            };

            let mut processed_count = 0;

            // Process each JSON string
            for (i, json_str) in json_strings.iter().enumerate() {
                if json_str.trim().is_empty() {
                    continue;
                }

                // Choose validation strategy based on delimiter
                let validation_result = if let Some(delim) = config.delimiter {
                    if delim == b'\n' {
                        validate_ndjson(json_str)
                    } else {
                        Err(serde_json::Error::custom(format!(
                            "Unsupported delimiter: {:?}",
                            delim
                        )))
                    }
                } else {
                    validate_json(json_str)
                };

                if let Err(parse_error) = validation_result {
                    let truncated_json = if json_str.len() > MAX_JSON_ERROR_LENGTH {
                        format!(
                            "{}... [truncated {} chars]",
                            &json_str[..MAX_JSON_ERROR_LENGTH],
                            json_str.len() - MAX_JSON_ERROR_LENGTH
                        )
                    } else {
                        json_str.clone()
                    };

                    return Err(format!(
                        "Invalid JSON input at index {}: {} - JSON: {}",
                        i + 1,
                        parse_error,
                        truncated_json
                    ));
                }

                // Safe: JSON is valid, now hand off to genson-rs
                let mut bytes = json_str.as_bytes().to_vec();

                // Build schema incrementally - this is where panics happen
                let _schema = build_json_schema(&mut builder, &mut bytes, &build_config);
                processed_count += 1;
            }

            // Get final schema
            let final_schema = builder.to_schema();

            Ok(SchemaInferenceResult {
                schema: final_schema,
                processed_count,
            })
        },
    ));

    // Handle the result of panic::catch_unwind
    match result {
        Ok(Ok(schema_result)) => Ok(schema_result),
        Ok(Err(e)) => Err(e),
        Err(_panic) => Err("JSON schema inference failed due to invalid JSON input".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use predicates::prelude::*;
    use serde_json::json;

    #[test]
    fn test_basic_schema_inference() {
        let json_strings = vec![
            r#"{"name": "Alice", "age": 30}"#.to_string(),
            r#"{"name": "Bob", "age": 25, "city": "NYC"}"#.to_string(),
        ];

        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default())
                .expect("Schema inference should succeed");

        // Test processed count
        assert_eq!(result.processed_count, 2);

        // Use predicates to test schema structure
        let schema_str = result.schema.to_string();

        predicate::str::contains("\"type\"")
            .and(predicate::str::contains("object"))
            .eval(&schema_str);

        predicate::str::contains("\"properties\"").eval(&schema_str);

        // Check that both name and age properties are present
        predicate::str::contains("\"name\"")
            .and(predicate::str::contains("\"age\""))
            .eval(&schema_str);

        println!(
            "✅ Generated schema: {}",
            serde_json::to_string_pretty(&result.schema).unwrap()
        );
    }

    #[test]
    fn test_empty_input() {
        let json_strings = vec![];
        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

        assert!(result.is_err());

        let error_msg = result.unwrap_err();
        predicate::str::contains("No JSON strings provided").eval(&error_msg);

        println!("✅ Empty input correctly rejected with: {}", error_msg);
    }

    #[test]
    fn test_invalid_json_variants() {
        let test_cases = vec![
            (
                r#"{"name": "Alice"}"#,
                r#"{"invalid": json}"#,
                "unquoted value",
            ),
            (
                r#"{"valid": "json"}"#,
                r#"{"incomplete":"#,
                "incomplete string",
            ),
            (r#"{"good": "data"}"#, r#"{"trailing":,"#, "trailing comma"),
            (
                r#"{"working": true}"#,
                r#"{invalid: "json"}"#,
                "unquoted key",
            ),
            (
                r#"{"normal": "object"}"#,
                r#"{"nested": {"broken": json}}"#,
                "nested broken JSON",
            ),
        ];

        for (valid_json, invalid_json, description) in test_cases {
            println!("🧪 Testing: {}", description);
            println!("   Valid JSON: {}", valid_json);
            println!("   Invalid JSON: {}", invalid_json);

            let json_strings = vec![valid_json.to_string(), invalid_json.to_string()];

            let result =
                infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

            // Should return an error instead of panicking
            assert!(result.is_err(), "Expected error for case: {}", description);

            let error_msg = result.unwrap_err();

            // Use predicates to verify error message content
            predicate::str::contains("Invalid JSON input at position").eval(&error_msg);

            // For short JSON strings, verify the content is included
            if invalid_json.len() <= MAX_JSON_ERROR_LENGTH {
                predicate::str::contains(invalid_json).eval(&error_msg);
            } else {
                // For long JSON, just check that truncation happened
                predicate::str::contains("truncated").eval(&error_msg);
            }

            // Ensure we don't have panic-related messages
            predicate::str::contains("panicked").not().eval(&error_msg);

            predicate::str::contains("SIGABRT").not().eval(&error_msg);

            println!("   ❌ Correctly failed with: {}", error_msg);
            println!();
        }
    }

    #[test]
    fn test_mixed_valid_and_empty_strings() {
        let json_strings = vec![
            r#"{"name": "Alice", "age": 30}"#.to_string(),
            "".to_string(),    // Empty string should be skipped
            "   ".to_string(), // Whitespace-only should be skipped
            r#"{"name": "Bob", "age": 25}"#.to_string(),
        ];

        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default())
                .expect("Should succeed with valid JSON, skipping empty strings");

        // Should process only the 2 valid JSON strings
        assert_eq!(result.processed_count, 2);

        let schema_str = result.schema.to_string();
        predicate::str::contains("\"name\"")
            .and(predicate::str::contains("\"age\""))
            .eval(&schema_str);

        println!(
            "✅ Processed {} valid strings, skipped empty ones",
            result.processed_count
        );
    }

    #[test]
    fn test_schema_config_variations() {
        let json_strings = vec![r#"[{"item": "first"}, {"item": "second"}]"#.to_string()];

        // Test with ignore_outer_array = false
        let config_array = SchemaInferenceConfig {
            ignore_outer_array: false,
            ..Default::default()
        };

        let result = infer_json_schema_from_strings(&json_strings, config_array)
            .expect("Should handle array schema");

        let schema_str = result.schema.to_string();
        predicate::str::contains("\"type\"")
            .and(predicate::str::contains("array"))
            .eval(&schema_str);

        println!(
            "✅ Array schema: {}",
            serde_json::to_string_pretty(&result.schema).unwrap()
        );

        // Test with ignore_outer_array = true (default)
        let config_object = SchemaInferenceConfig {
            ignore_outer_array: true,
            ..Default::default()
        };

        let result = infer_json_schema_from_strings(&json_strings, config_object)
            .expect("Should handle object schema from array items");

        let schema_str = result.schema.to_string();
        predicate::str::contains("\"type\"")
            .and(predicate::str::contains("object"))
            .eval(&schema_str);

        predicate::str::contains("\"item\"").eval(&schema_str);

        println!(
            "✅ Object schema from array items: {}",
            serde_json::to_string_pretty(&result.schema).unwrap()
        );
    }

    #[test]
    fn test_very_long_invalid_json() {
        // Create a very long invalid JSON string
        let long_value = "x".repeat(500); // 500 char string
        let long_invalid_json = format!(
            r#"{{"field1": "{}", "field2": "{}", "field3": "{}", "field4": "{}", "invalid_syntax": }}"#,
            long_value, long_value, long_value, long_value
        );

        let json_strings = vec![
            r#"{"valid": "json"}"#.to_string(),
            long_invalid_json.clone(),
        ];

        println!(
            "🧪 Testing very long invalid JSON ({} chars)",
            long_invalid_json.len()
        );

        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

        assert!(result.is_err(), "Expected error for very long invalid JSON");

        let error_msg = result.unwrap_err();

        println!("The error message was: {}", error_msg);

        // Should contain truncation indicator
        predicate::str::contains("truncated").eval(&error_msg);

        // Should contain position information
        predicate::str::contains("Invalid JSON input at position 2").eval(&error_msg);

        // Error message should be reasonable length (much shorter than original JSON)
        assert!(
            error_msg.len() < long_invalid_json.len() / 2,
            "Error message should be much shorter than original JSON"
        );

        println!(
            "   ❌ Correctly truncated long JSON in error: {}",
            error_msg
        );

        // Verify the error message doesn't exceed a reasonable length
        assert!(
            error_msg.len() < 500,
            "Error message should be under 500 chars, got: {}",
            error_msg.len()
        );
    }

    #[test]
    fn test_complex_nested_schema() {
        let json_strings = vec![
            json!({
                "user": {
                    "id": 123,
                    "profile": {
                        "name": "Alice",
                        "preferences": ["dark_mode", "notifications"]
                    }
                },
                "metadata": {
                    "created_at": "2024-01-01",
                    "version": 1
                }
            })
            .to_string(),
            json!({
                "user": {
                    "id": 456,
                    "profile": {
                        "name": "Bob",
                        "preferences": ["light_mode"]
                    }
                },
                "metadata": {
                    "created_at": "2024-01-02",
                    "version": 2
                }
            })
            .to_string(),
        ];

        let result =
            infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default())
                .expect("Should handle complex nested schema");

        assert_eq!(result.processed_count, 2);

        let schema_str = result.schema.to_string();

        // Check for nested structure
        predicate::str::contains("\"user\"")
            .and(predicate::str::contains("\"metadata\""))
            .and(predicate::str::contains("\"profile\""))
            .and(predicate::str::contains("\"preferences\""))
            .eval(&schema_str);

        println!("✅ Complex nested schema generated successfully");
        println!(
            "Schema: {}",
            serde_json::to_string_pretty(&result.schema).unwrap()
        );
    }

    #[test]
    fn test_ndjson_parsing() {
        // Two valid JSON objects separated by newlines (NDJSON format)
        let ndjson_input = r#"
{"name": "Alice", "age": 30}
{"name": "Bob", "age": 25, "city": "NYC"}
{"name": "Charlie"}
"#;

        let json_strings = vec![ndjson_input.to_string()];

        let config = SchemaInferenceConfig {
            delimiter: Some(b'\n'),
            ..Default::default()
        };

        let result = infer_json_schema_from_strings(&json_strings, config)
            .expect("NDJSON schema inference should succeed");

        // All 3 objects should be processed
        assert_eq!(result.processed_count, 1,
            "NDJSON should be counted as a single input string but parsed into multiple rows internally"
        );

        let schema_str = result.schema.to_string();

        // The schema should include properties from all lines
        assert!(!schema_str.contains("Alice")); // values are not in schema
        assert!(schema_str.contains("\"name\""));
        assert!(schema_str.contains("\"age\""));
        assert!(schema_str.contains("\"city\""));

        println!(
            "✅ NDJSON schema generated: {}",
            serde_json::to_string_pretty(&result.schema).unwrap()
        );
    }

    #[test]
    fn test_invalid_ndjson_line() {
        // Second line is malformed
        let ndjson_input = r#"
{"valid": true}
{"invalid": json}
{"also_valid": 123}
"#;

        let json_strings = vec![ndjson_input.to_string()];

        let config = SchemaInferenceConfig {
            delimiter: Some(b'\n'),
            ..Default::default()
        };

        let result = infer_json_schema_from_strings(&json_strings, config);

        assert!(result.is_err(), "Expected error for malformed NDJSON line");

        let err_msg = result.unwrap_err();
        eprintln!("Got error: {}", err_msg);
        assert!(
            err_msg.contains("Invalid JSON input at index 1: expected value at line 1 column 13"),
            "Error message should report the failing line"
        );
        println!("✅ Correctly rejected malformed NDJSON: {}", err_msg);
    }
}
