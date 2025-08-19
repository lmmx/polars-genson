use std::panic::{self, AssertUnwindSafe};
use genson_rs::{build_json_schema, get_builder, BuildConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Infer JSON schema from a collection of JSON strings
pub fn infer_schema_from_strings(
    json_strings: &[String],
    config: SchemaInferenceConfig,
) -> Result<SchemaInferenceResult, String> {
    eprintln!("DEBUG: infer_schema_from_strings called with {} strings", json_strings.len());

    if json_strings.is_empty() {
        return Err("No JSON strings provided".to_string());
    }

    // Wrap the entire genson-rs interaction in panic handling
    let result = panic::catch_unwind(AssertUnwindSafe(|| -> Result<SchemaInferenceResult, String> {
        // Create schema builder
        let mut builder = get_builder(config.schema_uri.as_deref());

        // Build config for genson-rs
        let build_config = BuildConfig {
            delimiter: config.delimiter,
            ignore_outer_array: config.ignore_outer_array,
        };

        let mut processed_count = 0;

        // Process each JSON string
        for json_str in json_strings {
            if json_str.trim().is_empty() {
                continue;
            }

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
    }));

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

    #[test]
    fn test_basic_schema_inference() {
        let json_strings = vec![
            r#"{"name": "Alice", "age": 30}"#.to_string(),
            r#"{"name": "Bob", "age": 25, "city": "NYC"}"#.to_string(),
        ];

        let result = infer_schema_from_strings(&json_strings, SchemaInferenceConfig::default())
            .expect("Schema inference should succeed");

        assert_eq!(result.processed_count, 2);
        assert!(result.schema.is_object());
    }

    #[test]
    fn test_empty_input() {
        let json_strings = vec![];
        let result = infer_schema_from_strings(&json_strings, SchemaInferenceConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json() {
        let json_strings = vec![
            r#"{"name": "Alice"}"#.to_string(),
            r#"{"invalid": json}"#.to_string(), // This should cause a panic in genson-rs
            r#"{"name": "Bob"}"#.to_string(),
        ];

        let result = infer_schema_from_strings(&json_strings, SchemaInferenceConfig::default());
        
        // Should return an error instead of panicking
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid JSON"));
    }
}
