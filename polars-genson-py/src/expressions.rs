use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use polars::prelude::*;
use polars_jsonschema_bridge::deserialise::json_schema_to_polars_fields;
use polars_jsonschema_bridge::serialise::{polars_schema_to_json_schema, JsonSchemaOptions};
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

#[derive(Deserialize)]
pub struct SerializeSchemaKwargs {
    #[serde(default)]
    pub schema_uri: Option<String>,

    #[serde(default)]
    pub title: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub optional_fields: Vec<String>,

    #[serde(default)]
    pub additional_properties: bool,

    #[serde(default)]
    pub debug: bool,
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

/// Serialized schema is a String (JSON)
fn serialize_schema_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new("json_schema".into(), DataType::String))
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
        let polars_fields = json_schema_to_polars_fields(&schema_result.schema, kwargs.debug)
            .map_err(|e| e.to_string())?;
        Ok(polars_fields)
    });

    match result {
        Ok(Ok(polars_fields)) => {
            // Convert field mappings to name/dtype series
            let field_names: Vec<String> =
                polars_fields.iter().map(|(name, _)| name.clone()).collect();
            let field_dtypes: Vec<String> = polars_fields
                .iter()
                .map(|(_, dtype)| dtype.clone())
                .collect();

            let names = Series::new("name".into(), field_names);
            let dtypes = Series::new("dtype".into(), field_dtypes);

            // Create struct series
            let struct_series = StructChunked::from_series(
                "schema_field".into(),
                names.len(),
                [&names, &dtypes].iter().cloned(),
            )?
            .into_series();

            // Create list for each input row
            let list_values: Vec<Series> =
                (0..series.len()).map(|_| struct_series.clone()).collect();

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

/// Polars expression that serializes schema fields to JSON Schema
/// Takes a series of struct columns representing schema fields
#[polars_expr(output_type_func=serialize_schema_output_type)]
pub fn serialize_polars_schema(
    inputs: &[Series],
    kwargs: SerializeSchemaKwargs,
) -> PolarsResult<Series> {
    if inputs.is_empty() {
        return Err(PolarsError::ComputeError("No input series provided".into()));
    }

    let series = &inputs[0];

    // Expect a struct series with "name" and "dtype" fields representing the schema
    let struct_chunked = series.struct_().map_err(|_| {
        PolarsError::ComputeError("Expected a struct column with schema field information".into())
    })?;

    // Extract the schema fields from the struct
    let fields = struct_chunked.fields_as_series();

    if fields.len() != 2 {
        return Err(PolarsError::ComputeError(
            "Expected struct with exactly 2 fields: 'name' and 'dtype'".into(),
        ));
    }

    // Get the name and dtype columns
    let name_series = &fields[0];
    let dtype_series = &fields[1];

    let name_chunked = name_series
        .str()
        .map_err(|_| PolarsError::ComputeError("Expected 'name' field to be string type".into()))?;

    let dtype_chunked = dtype_series.str().map_err(|_| {
        PolarsError::ComputeError("Expected 'dtype' field to be string type".into())
    })?;

    // Build a Polars Schema from the field information
    let mut polars_schema = Schema::default();

    for (name_opt, dtype_opt) in name_chunked.iter().zip(dtype_chunked.iter()) {
        if let (Some(name), Some(dtype_str)) = (name_opt, dtype_opt) {
            // Parse the dtype string back to a DataType
            // This is a simplified version - you might want to implement a more complete parser
            let polars_dtype = match dtype_str {
                "String" => DataType::String,
                "Int64" => DataType::Int64,
                "Int32" => DataType::Int32,
                "Float64" => DataType::Float64,
                "Float32" => DataType::Float32,
                "Boolean" => DataType::Boolean,
                "Date" => DataType::Date,
                "Time" => DataType::Time,
                s if s.starts_with("List[") && s.ends_with("]") => {
                    let inner_type = &s[5..s.len() - 1];
                    match inner_type {
                        "String" => DataType::List(Box::new(DataType::String)),
                        "Int64" => DataType::List(Box::new(DataType::Int64)),
                        "Float64" => DataType::List(Box::new(DataType::Float64)),
                        "Boolean" => DataType::List(Box::new(DataType::Boolean)),
                        _ => DataType::String, // Fallback
                    }
                }
                _ => DataType::String, // Fallback for unknown types
            };

            polars_schema.with_column(name.into(), polars_dtype);
        }
    }

    if kwargs.debug {
        eprintln!("DEBUG: Polars schema to serialize: {:?}", polars_schema);
    }

    // Create JsonSchemaOptions from kwargs
    let mut options = JsonSchemaOptions::new();

    if let Some(uri) = kwargs.schema_uri {
        options = options.with_schema_uri(Some(uri));
    }

    if let Some(title) = kwargs.title {
        options = options.with_title(Some(title));
    }

    if let Some(description) = kwargs.description {
        options = options.with_description(Some(description));
    }

    if !kwargs.optional_fields.is_empty() {
        options = options.with_optional_fields(kwargs.optional_fields);
    }

    options = options.with_additional_properties(kwargs.additional_properties);

    // Convert to JSON Schema using the bridge
    let result = panic::catch_unwind(|| -> Result<String, String> {
        let json_schema = polars_schema_to_json_schema(&polars_schema, &options)
            .map_err(|e| format!("Schema serialization error: {}", e))?;

        serde_json::to_string_pretty(&json_schema)
            .map_err(|e| format!("JSON serialization error: {}", e))
    });

    match result {
        Ok(Ok(json_schema_str)) => {
            if kwargs.debug {
                eprintln!("DEBUG: Generated JSON Schema:");
                eprintln!("{}", json_schema_str);
            }
            Ok(Series::new(
                "json_schema".into(),
                vec![json_schema_str; series.len()],
            ))
        }
        Ok(Err(e)) => Err(PolarsError::ComputeError(
            format!("Schema serialization failed: {}", e).into(),
        )),
        Err(_panic) => Err(PolarsError::ComputeError(
            "Panic occurred during schema serialization".into(),
        )),
    }
}
