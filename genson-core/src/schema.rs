use crate::debug;
use crate::genson_rs::{build_json_schema, get_builder, BuildConfig};
use rayon::prelude::*;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::time::{SystemTime, UNIX_EPOCH};
use xxhash_rust::xxh64::xxh64;

use crate::genson_rs::SchemaBuilder;

pub(crate) mod core;
pub use core::*;
mod map_inference;
use map_inference::*;

/// Maximum length of JSON string to include in error messages before truncating
const MAX_JSON_ERROR_LENGTH: usize = 100;
/// Threshold for switching to parallel processing. Below this, use sequential.
const PARALLEL_THRESHOLD: usize = 10;

fn current_time_hms() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let total_seconds = now.as_secs() % 86_400; // seconds in a day
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
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

/// Prepare a single JSON string for schema building (validation + wrap_root transformation)
fn prepare_json_string(
    json_str: &str,
    index: usize,
    config: &SchemaInferenceConfig,
) -> Result<String, String> {
    if json_str.trim().is_empty() {
        return Ok(String::new());
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
            json_str.to_string()
        };

        return Err(format!(
            "Invalid JSON input at index {}: {} - JSON: {}",
            index + 1,
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
                let inner_val: Value = serde_json::from_str(trimmed)
                    .map_err(|e| format!("Failed to parse NDJSON line before wrap_root: {}", e))?;
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

    Ok(prepared_json.into_owned())
}

/// Process all JSON strings sequentially and build schemas
fn process_json_strings_sequential(
    json_strings: &[String],
    config: &SchemaInferenceConfig,
    builder: &mut crate::genson_rs::SchemaBuilder,
) -> Result<usize, String> {
    let build_config = BuildConfig {
        delimiter: config.delimiter,
        ignore_outer_array: config.ignore_outer_array,
    };

    let mut processed_count = 0;

    // Process each JSON string
    for (i, json_str) in json_strings.iter().enumerate() {
        eprintln!("PROCESSING JSON STRING {}", i);

        let prep_start = std::time::Instant::now();
        let prepared_json = prepare_json_string(json_str, i, config)?;
        let prep_elapsed = prep_start.elapsed();
        eprintln!("  Preparation took: {:?}", prep_elapsed);

        if prepared_json.is_empty() {
            continue;
        }

        let mut bytes = prepared_json.as_bytes().to_vec();

        // Build schema incrementally - this is where panics happen
        let build_start = std::time::Instant::now();
        let _schema = build_json_schema(builder, &mut bytes, &build_config);
        let build_elapsed = build_start.elapsed();
        eprintln!("  Schema building took: {:?}", build_elapsed);

        processed_count += 1;
    }

    Ok(processed_count)
}

/// Process all JSON strings in parallel while maintaining order
fn process_json_strings_parallel(
    json_strings: &[String],
    config: &SchemaInferenceConfig,
    builder: &mut SchemaBuilder,
) -> Result<usize, String> {
    eprintln!(
        "Starting parallel preparation and building ({})",
        current_time_hms()
    );

    // Process each string independently in parallel, keeping track of index
    let individual_builders: Vec<(usize, SchemaBuilder, bool)> = json_strings
        .par_iter()
        .enumerate()
        .map(
            |(i, json_str)| -> Result<(usize, SchemaBuilder, bool), String> {
                eprintln!("Thread processing JSON STRING {}", i);

                let prep_start = std::time::Instant::now();
                let prepared = prepare_json_string(json_str, i, config)?;
                let prep_elapsed = prep_start.elapsed();
                eprintln!("  String {} preparation took: {:?}", i, prep_elapsed);

                if prepared.is_empty() {
                    return Ok((i, get_builder(config.schema_uri.as_deref()), false));
                    // Mark as empty
                }

                let mut chunk_builder = get_builder(config.schema_uri.as_deref());
                let mut bytes = prepared.as_bytes().to_vec();
                let chunk_build_config = BuildConfig {
                    delimiter: config.delimiter,
                    ignore_outer_array: config.ignore_outer_array,
                };

                let build_start = std::time::Instant::now();
                build_json_schema(&mut chunk_builder, &mut bytes, &chunk_build_config);
                let build_elapsed = build_start.elapsed();
                eprintln!("  String {} schema building took: {:?}", i, build_elapsed);

                Ok((i, chunk_builder, true)) // Mark as non-empty
            },
        )
        .collect::<Result<Vec<_>, String>>()?;

    eprintln!(
        "Merging schemas sequentially in order ({})",
        current_time_hms()
    );

    // Merge sequentially in the original order, only counting non-empty
    let mut processed_count = 0;
    let mut seen_hashes = HashSet::new();
    for (i, individual_builder, was_non_empty) in individual_builders.into_iter() {
        if was_non_empty {
            let schema = individual_builder.to_schema();
            // Quick hash check
            let schema_str = schema.to_string();
            let hash = xxh64(schema_str.as_bytes(), 0);
            // Only add the schema if we haven't seen an identical one before
            if seen_hashes.insert(hash) {
                builder.add_schema(schema);
                eprintln!("  Merged schema {} ({})", i, current_time_hms());
            } else {
                eprintln!("  Schema {} already merged ({})", i, current_time_hms());
            }
            processed_count += 1;
        }
    }

    eprintln!("Sequential merge complete ({})", current_time_hms());

    Ok(processed_count)
}

/// Infer JSON schema from a collection of JSON strings
pub fn infer_json_schema_from_strings(
    json_strings: &[String],
    config: SchemaInferenceConfig,
) -> Result<SchemaInferenceResult, String> {
    eprintln!(
        "Processing {} strings ({})",
        json_strings.len(),
        current_time_hms()
    );
    debug!(config, "Schema inference config: {:#?}", config);
    if json_strings.is_empty() {
        return Err("No JSON strings provided".to_string());
    }

    // Wrap the entire genson-rs interaction in panic handling
    let result = panic::catch_unwind(AssertUnwindSafe(
        || -> Result<SchemaInferenceResult, String> {
            // Create schema builder
            let mut builder = get_builder(config.schema_uri.as_deref());

            eprintln!("Starting preparation loop ({})", current_time_hms());

            let use_parallel = std::env::var("GENSON_PARALLEL")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or_else(|_| json_strings.len() >= PARALLEL_THRESHOLD);

            let processed_count = if use_parallel {
                process_json_strings_parallel(json_strings, &config, &mut builder)?
            } else {
                process_json_strings_sequential(json_strings, &config, &mut builder)?
            };

            // Get final schema
            let mut final_schema = builder.to_schema();
            eprintln!("Rewriting objects ({})", current_time_hms());
            rewrite_objects(&mut final_schema, None, &config, true);
            eprintln!("Reordering unions ({})", current_time_hms());
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
