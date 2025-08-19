use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;
use std::panic;

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

    #[allow(dead_code)]
    #[serde(default)]
    pub convert_to_polars: bool,
}

fn default_ignore_outer_array() -> bool {
    true
}

fn default_merge_schemas() -> bool {
    true
}

/// JSON Schema is a String
fn infer_json_schema_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new("schema".into(), DataType::String))
}

/// Polars schema is serialised to String
fn infer_polars_schema_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    let schema_field_struct = DataType::Struct(vec![
        Field::new("name".into(), DataType::String),
        Field::new("dtype".into(), DataType::String),
    ]);
    Ok(Field::new(
        "schema".into(),
        DataType::List(Box::new(schema_field_struct)),
    ))
}

/// Polars expression that infers JSON schema from string column
#[polars_expr(output_type_func=infer_json_schema_output_type)]
pub fn infer_json_schema(inputs: &[Series], kwargs: GensonKwargs) -> PolarsResult<Series> {
    if inputs.is_empty() {
        return Err(PolarsError::ComputeError("No input series provided".into()));
    }

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

    if kwargs.debug {
        eprintln!("DEBUG: Processing {} JSON strings", json_strings.len());
        eprintln!(
            "DEBUG: Config: ignore_outer_array={}, ndjson={}",
            kwargs.ignore_outer_array, kwargs.ndjson
        );
        for (i, json_str) in json_strings.iter().take(3).enumerate() {
            eprintln!("DEBUG: Sample JSON {}: {}", i + 1, json_str);
        }
    }

    if kwargs.merge_schemas {
        // Original behavior: merge all schemas into one
        // Wrap EVERYTHING in panic catching, including config creation
        let result = panic::catch_unwind(|| -> Result<String, String> {
            let config = SchemaInferenceConfig {
                ignore_outer_array: kwargs.ignore_outer_array,
                delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
                schema_uri: kwargs.schema_uri.clone(),
            };

            let schema_result = infer_json_schema_from_strings(&json_strings, config)
                .map_err(|e| format!("Genson error: {}", e))?;

            serde_json::to_string_pretty(&schema_result.schema)
                .map_err(|e| format!("JSON serialization error: {}", e))
        });

        match result {
            Ok(Ok(schema_json)) => {
                if kwargs.debug {
                    eprintln!("DEBUG: Successfully generated merged schema");
                }
                Ok(Series::new(
                    "schema".into(),
                    vec![schema_json; series.len()],
                ))
            }
            Ok(Err(e)) => Err(PolarsError::ComputeError(
                format!("Merged schema processing failed: {}", e).into(),
            )),
            Err(_panic) => Err(PolarsError::ComputeError(
                "Panic occurred during merged schema JSON processing".into(),
            )),
        }
    } else {
        // New behavior: infer schema for each row individually
        let result = panic::catch_unwind(|| -> Result<Vec<serde_json::Value>, String> {
            let mut individual_schemas = Vec::new();
            for json_str in &json_strings {
                let config = SchemaInferenceConfig {
                    ignore_outer_array: kwargs.ignore_outer_array,
                    delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
                    schema_uri: kwargs.schema_uri.clone(),
                };

                let single_result = infer_json_schema_from_strings(&[json_str.clone()], config)
                    .map_err(|e| format!("Individual genson error: {}", e))?;
                individual_schemas.push(single_result.schema);
            }
            Ok(individual_schemas)
        });

        match result {
            Ok(Ok(individual_schemas)) => {
                if kwargs.debug {
                    eprintln!(
                        "DEBUG: Generated {} individual schemas",
                        individual_schemas.len()
                    );
                }

                // Return array of schemas as JSON
                let schemas_json =
                    serde_json::to_string_pretty(&individual_schemas).map_err(|e| {
                        PolarsError::ComputeError(
                            format!("Failed to serialize individual schemas: {}", e).into(),
                        )
                    })?;

                Ok(Series::new(
                    "schema".into(),
                    vec![schemas_json; series.len()],
                ))
            }
            Ok(Err(e)) => Err(PolarsError::ComputeError(
                format!("Individual schema inference failed: {}", e).into(),
            )),
            Err(_panic) => Err(PolarsError::ComputeError(
                "Panic occurred during individual schema inference".into(),
            )),
        }
    }
}

/// Polars expression that infers Polars schema from string column
#[polars_expr(output_type_func=infer_polars_schema_output_type)]
pub fn infer_polars_schema(inputs: &[Series], kwargs: GensonKwargs) -> PolarsResult<Series> {
    if inputs.is_empty() {
        return Err(PolarsError::ComputeError("No input series provided".into()));
    }

    let series = &inputs[0];
    let string_chunked = series.str().map_err(|_| {
        PolarsError::ComputeError("Expected a string column for Polars schema inference".into())
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

    // Use genson to infer JSON schema, then convert to Polars schema fields
    let result = panic::catch_unwind(|| -> Result<Vec<(String, String)>, String> {
        let config = SchemaInferenceConfig {
            ignore_outer_array: kwargs.ignore_outer_array,
            delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
            schema_uri: kwargs.schema_uri.clone(),
        };

        let schema_result = infer_json_schema_from_strings(&json_strings, config)
            .map_err(|e| format!("Genson error: {}", e))?;

        // Convert JSON schema to Polars field mappings
        let polars_fields = json_schema_to_polars_fields(&schema_result.schema)?;
        Ok(polars_fields)
    });

    match result {
        Ok(Ok(polars_fields)) => {
            // Convert field mappings to name/dtype series
            let field_names: Vec<String> = polars_fields.iter().map(|(name, _)| name.clone()).collect();
            let field_dtypes: Vec<String> = polars_fields.iter().map(|(_, dtype)| dtype.clone()).collect();

            let names = Series::new("name".into(), field_names);
            let dtypes = Series::new("dtype".into(), field_dtypes);

            // Create struct series
            let struct_series = StructChunked::from_series(
                "schema_field".into(),
                names.len(),
                [&names, &dtypes].iter().cloned(),
            )?.into_series();

            // Create list for each input row
            let list_values: Vec<Series> = (0..series.len())
                .map(|_| struct_series.clone())
                .collect();

            let list_series = Series::new("schema".into(), list_values);
            Ok(list_series)
        }
        Ok(Err(e)) => Err(PolarsError::ComputeError(
            format!("Schema conversion failed: {}", e).into(),
        )),
        Err(_panic) => Err(PolarsError::ComputeError(
            "Panic occurred during schema inference".into(),
        )),
    }
}

// Helper function to convert JSON schema to Polars field types
fn json_schema_to_polars_fields(json_schema: &serde_json::Value) -> Result<Vec<(String, String)>, String> {
    let mut fields = Vec::new();

    // Debug: print the full JSON schema
    eprintln!("=== Generated JSON Schema ===");
    eprintln!("{}", serde_json::to_string_pretty(json_schema).unwrap_or_else(|_| "Failed to serialize".to_string()));
    eprintln!("=============================");

    if let Some(properties) = json_schema.get("properties").and_then(|p| p.as_object()) {
        for (field_name, field_schema) in properties {
            let polars_type = json_type_to_polars_type(field_schema)?;
            fields.push((field_name.clone(), polars_type));
        }
    }

    Ok(fields)
}

fn json_type_to_polars_type(json_schema: &serde_json::Value) -> Result<String, String> {
    if let Some(type_value) = json_schema.get("type") {
        let type_repr = type_value.as_str();
        eprintln!("-- Processing type_value {:?}", type_repr);
        match type_repr {
            Some("string") => Ok("String".to_string()),
            Some("integer") => Ok("Int64".to_string()),
            Some("number") => Ok("Float64".to_string()),
            Some("boolean") => Ok("Boolean".to_string()),
            Some("array") => Ok("List".to_string()), // Simplified
            Some("object") => Ok("Struct".to_string()), // Simplified
            Some("null") => Ok("Null".to_string()),
            _ => Ok("String".to_string()), // Default fallback
        }
    } else {
        Ok("String".to_string()) // Default fallback
    }
}
