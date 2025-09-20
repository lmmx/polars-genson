use crate::debug;
use crate::genson_rs::{build_json_schema, get_builder, BuildConfig};
use serde::de::Error as DeError;
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};

pub(crate) mod core;
pub use core::*;
mod map_inference;
use map_inference::*;

/// Maximum length of JSON string to include in error messages before truncating
const MAX_JSON_ERROR_LENGTH: usize = 100;

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

/// Recursively reorder union type arrays in a JSON Schema by canonical precedence.
///
/// Special case: preserves the common `["null", T]` pattern without reordering.
pub fn reorder_unions(schema: &mut Value) {
    match schema {
        Value::Object(obj) => {
            if let Some(Value::Array(types)) = obj.get_mut("type") {
                // sort by canonical precedence, but keep ["null", T] pattern intact
                if !(types.len() == 2 && types.iter().any(|t| t == "null")) {
                    types.sort_by_key(type_rank);
                }
            }
            // recurse into properties/items/etc.
            for v in obj.values_mut() {
                reorder_unions(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                reorder_unions(v);
            }
        }
        _ => {}
    }
}

/// Assign a numeric precedence rank to a JSON Schema type.
///
/// Used by `reorder_unions` to sort union members deterministically.
/// - Null always first
/// - Containers before scalars (to enforce widening)
/// - Scalars ordered by narrowness
/// - Unknown types last
pub fn type_rank(val: &Value) -> usize {
    match val {
        Value::String(s) => type_string_rank(s),
        Value::Object(obj) => {
            if let Some(Value::String(t)) = obj.get("type") {
                type_string_rank(t)
            } else {
                100 // object with no "type" field
            }
        }
        _ => 100, // non-string/non-object
    }
}

/// Internal helper: rank by type string
fn type_string_rank(s: &str) -> usize {
    match s {
        // Null always first
        "null" => 0,

        // Containers before scalars: widening takes precedence
        "map" => 1,
        "array" => 2,
        "object" | "record" => 3,

        // Scalars (ordered by 'narrowness')
        "boolean" => 10,
        "integer" | "int" | "long" => 11,
        "number" | "float" | "double" => 12,
        "enum" => 13,
        "string" => 14,
        "fixed" => 15,
        "bytes" => 16,

        // Fallback
        _ => 99,
    }
}

/// Infer JSON schema from a collection of JSON strings
pub fn infer_json_schema_from_strings(
    json_strings: &[String],
    config: SchemaInferenceConfig,
) -> Result<SchemaInferenceResult, String> {
    debug!(config, "Schema inference config: {:#?}", config);
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
                let prepared_json: Cow<str> = if let Some(ref field) = config.wrap_root {
                    if config.delimiter == Some(b'\n') {
                        // NDJSON: wrap each line separately
                        let mut wrapped_lines = Vec::new();
                        for line in json_str.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let inner_val: Value = serde_json::from_str(trimmed).map_err(|e| {
                                format!("Failed to parse NDJSON line before wrap_root: {}", e)
                            })?;
                            wrapped_lines.push(serde_json::json!({ field: inner_val }).to_string());
                        }
                        Cow::Owned(wrapped_lines.join("\n"))
                    } else {
                        // Single JSON doc
                        let inner_val: Value = serde_json::from_str(json_str)
                            .map_err(|e| format!("Failed to parse JSON before wrap_root: {}", e))?;
                        Cow::Owned(serde_json::json!({ field: inner_val }).to_string())
                    }
                } else {
                    Cow::Borrowed(json_str)
                };

                let mut bytes = prepared_json.as_bytes().to_vec();

                // Build schema incrementally - this is where panics happen
                let _schema = build_json_schema(&mut builder, &mut bytes, &build_config);
                processed_count += 1;
            }

            // Get final schema
            let mut final_schema = builder.to_schema();
            rewrite_objects(&mut final_schema, None, &config, true);
            reorder_unions(&mut final_schema);

            #[cfg(feature = "avro")]
            if config.avro {
                let avro_schema = SchemaInferenceResult {
                    schema: final_schema.clone(),
                    processed_count,
                }
                .to_avro_schema(
                    "genson", // namespace
                    Some(""),
                    Some(""), // base_uri
                    false,    // don't split top-level
                );
                return Ok(SchemaInferenceResult {
                    schema: avro_schema,
                    processed_count,
                });
            }

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
    include!("tests/schema.rs");
}
