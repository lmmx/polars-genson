use genson_core::{infer_schema_from_strings, SchemaInferenceConfig};
use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GensonKwargs {
    #[serde(default = "default_ignore_outer_array")]
    pub ignore_outer_array: bool,

    #[serde(default)]
    pub ndjson: bool,

    #[serde(default)]
    pub schema_uri: Option<String>,

    #[serde(default)]
    pub debug: bool,

    #[serde(default = "default_merge_schemas")]
    pub merge_schemas: bool,
}

fn default_ignore_outer_array() -> bool {
    true
}

fn default_merge_schemas() -> bool {
    true
}

/// Computes output type for the expression
fn infer_json_schema_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    // Return a JSON object (represented as string in Polars)
    // If merge_schemas=false, this will be a list of schemas
    Ok(Field::new("schema".into(), DataType::String))
}

/// Polars expression that infers JSON schema from string column
#[polars_expr(output_type_func=infer_json_schema_output_type)]
pub fn infer_json_schema(inputs: &[Series], kwargs: GensonKwargs) -> PolarsResult<Series> {
    let series = &inputs[0];

    // Ensure we have a string column
    let string_chunked = series.str().map_err(|_| {
        PolarsError::ComputeError("Expected a string column for JSON schema inference".into())
    })?;

    // Collect all non-null string values from ALL rows
    let mut json_strings = Vec::new();
    for s in string_chunked.iter().flatten() {
        if !s.trim().is_empty() {
            json_strings.push(s.to_string());
        }
    }

    if json_strings.is_empty() {
        return Err(PolarsError::ComputeError(
            "No valid JSON strings found in column".into(),
        ));
    }

    // Configure schema inference
    let config = SchemaInferenceConfig {
        ignore_outer_array: kwargs.ignore_outer_array,
        delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
        schema_uri: kwargs.schema_uri,
    };

    if kwargs.debug {
        eprintln!("DEBUG: Processing {} JSON strings", json_strings.len());
        eprintln!(
            "DEBUG: Config: ignore_outer_array={}, ndjson={}",
            config.ignore_outer_array, kwargs.ndjson
        );
        for (i, json_str) in json_strings.iter().take(3).enumerate() {
            eprintln!("DEBUG: Sample JSON {}: {}", i + 1, json_str);
        }
    }

    if kwargs.merge_schemas {
        // Original behavior: merge all schemas into one
        let result = infer_schema_from_strings(&json_strings, config).map_err(|e| {
            PolarsError::ComputeError(format!("Schema inference failed: {}", e).into())
        })?;

        if kwargs.debug {
            eprintln!("DEBUG: Processed {} objects", result.processed_count);
            eprintln!(
                "DEBUG: Merged schema: {}",
                serde_json::to_string_pretty(&result.schema)
                    .unwrap_or_else(|_| "Failed to serialize".to_string())
            );
        }

        // Convert schema to pretty JSON string
        let schema_json = serde_json::to_string_pretty(&result.schema).map_err(|e| {
            PolarsError::ComputeError(format!("Failed to serialize schema: {}", e).into())
        })?;

        // Create a series with the same length as input, but all values are the same schema
        let schema_values: Vec<Option<&str>> = vec![Some(schema_json.as_str()); series.len()];
        Ok(Series::new("schema".into(), schema_values))
    } else {
        // New behavior: infer schema for each row individually
        let mut individual_schemas = Vec::new();

        for json_str in &json_strings {
            let single_result = infer_schema_from_strings(&[json_str.clone()], config.clone())
                .map_err(|e| {
                    PolarsError::ComputeError(
                        format!("Schema inference failed for individual JSON: {}", e).into(),
                    )
                })?;
            individual_schemas.push(single_result.schema);
        }

        if kwargs.debug {
            eprintln!(
                "DEBUG: Generated {} individual schemas",
                individual_schemas.len()
            );
        }

        // Return array of schemas as JSON
        let schemas_json = serde_json::to_string_pretty(&individual_schemas).map_err(|e| {
            PolarsError::ComputeError(format!("Failed to serialize schemas: {}", e).into())
        })?;

        // For individual schemas, we could return per-row results, but for simplicity
        // we'll return the array of all schemas in each row
        let schema_values: Vec<Option<&str>> = vec![Some(schemas_json.as_str()); series.len()];
        Ok(Series::new("schema".into(), schema_values))
    }
}
