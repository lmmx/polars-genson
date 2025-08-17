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
}

fn default_ignore_outer_array() -> bool {
    true
}

/// Computes output type for the expression
fn infer_json_schema_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    // Return a JSON string representing the schema
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

    // Collect all non-null string values
    let mut json_strings = Vec::new();
    for opt_str in string_chunked.iter() {
        if let Some(s) = opt_str {
            if !s.trim().is_empty() {
                json_strings.push(s.to_string());
            }
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
    }

    // Infer schema using the core function
    let result = infer_schema_from_strings(&json_strings, config)
        .map_err(|e| PolarsError::ComputeError(format!("Schema inference failed: {}", e).into()))?;

    if kwargs.debug {
        eprintln!("DEBUG: Processed {} objects", result.processed_count);
    }

    // Convert schema to JSON string
    let schema_json = serde_json::to_string(&result.schema).map_err(|e| {
        PolarsError::ComputeError(format!("Failed to serialize schema: {}", e).into())
    })?;

    // Return as single-value series
    Ok(Series::new("schema".into(), &[schema_json]))
}
