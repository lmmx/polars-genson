use genson_core::normalise::{normalise_values, MapEncoding, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, DebugVerbosity, SchemaInferenceConfig};
use polars::prelude::*;
use polars_jsonschema_bridge::deserialise::{schema_to_polars_fields, SchemaFormat};
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;
use std::panic;
use std::slice::from_ref;

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

    #[serde(default)]
    pub verbosity: DebugVerbosity,

    #[serde(default = "default_merge_schemas")]
    pub merge_schemas: bool,

    #[allow(dead_code)]
    #[serde(default)]
    pub convert_to_polars: bool,

    #[serde(default = "default_map_threshold")]
    pub map_threshold: usize,

    #[serde(default)]
    pub map_max_required_keys: Option<usize>,

    /// Enable unification of compatible but non-homogeneous record schemas into maps
    #[serde(default)]
    pub unify_maps: bool,

    #[serde(default)]
    pub no_unify: Vec<String>,

    #[serde(default)]
    pub force_field_types: std::collections::HashMap<String, String>,

    #[serde(default = "default_wrap_scalars")]
    pub wrap_scalars: bool,

    /// Whether to emit Avro schema instead of JSON Schema
    #[serde(default)]
    pub avro: bool,

    #[serde(default = "default_empty_as_null")]
    pub empty_as_null: bool,

    #[serde(default)]
    pub coerce_string: bool,

    /// Map encoding strategy (default: KeyValueEntries for Polars friendliness)
    #[serde(default = "default_map_encoding")]
    pub map_encoding: MapEncoding,

    /// Wrap the root object under a single field.
    ///
    /// - If set to `Some("field")`, all input JSON objects are wrapped inside
    ///   an object with key `"field"`.
    #[serde(default)]
    pub wrap_root: Option<String>,

    #[serde(default = "default_no_root_map")]
    pub no_root_map: bool,
}

fn default_map_threshold() -> usize {
    20
}

fn default_ignore_outer_array() -> bool {
    true
}

fn default_merge_schemas() -> bool {
    true
}

fn default_empty_as_null() -> bool {
    true
}

fn default_wrap_scalars() -> bool {
    true
}

fn default_no_root_map() -> bool {
    true
}

fn default_map_encoding() -> MapEncoding {
    MapEncoding::KeyValueEntries
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

/// Normalised JSON is still a JSON string
fn normalise_json_output_type(_input_fields: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new("normalised".into(), DataType::String))
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

    let wrap_root_field = kwargs.wrap_root.clone();

    if kwargs.merge_schemas {
        // Original behavior: merge all schemas into one
        // We only need a single row, and we are allowed to change the length
        // Wrap EVERYTHING in panic catching, including config creation
        let result = panic::catch_unwind(|| -> Result<String, String> {
            let config = SchemaInferenceConfig {
                ignore_outer_array: kwargs.ignore_outer_array,
                delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
                schema_uri: kwargs.schema_uri.clone(),
                map_threshold: kwargs.map_threshold,
                map_max_required_keys: kwargs.map_max_required_keys,
                unify_maps: kwargs.unify_maps,
                no_unify: kwargs.no_unify.iter().cloned().collect(),
                force_field_types: kwargs.force_field_types.clone(),
                wrap_scalars: kwargs.wrap_scalars,
                avro: kwargs.avro,
                wrap_root: wrap_root_field.clone(),
                no_root_map: kwargs.no_root_map,
                debug: kwargs.debug,
                verbosity: kwargs.verbosity,
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
                Ok(Series::new("schema".into(), vec![schema_json; 1]))
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
                    map_threshold: kwargs.map_threshold,
                    map_max_required_keys: kwargs.map_max_required_keys,
                    unify_maps: kwargs.unify_maps,
                    no_unify: kwargs.no_unify.iter().cloned().collect(),
                    force_field_types: kwargs.force_field_types.clone(),
                    wrap_scalars: kwargs.wrap_scalars,
                    avro: kwargs.avro,
                    wrap_root: wrap_root_field.clone(),
                    no_root_map: kwargs.no_root_map,
                    debug: kwargs.debug,
                    verbosity: kwargs.verbosity,
                };

                let single_result = infer_json_schema_from_strings(from_ref(json_str), config)
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

    let wrap_root_field = kwargs.wrap_root.clone();

    // Use genson to infer JSON schema, then convert to Polars schema fields
    let result = panic::catch_unwind(|| -> Result<Vec<(String, String)>, String> {
        let config = SchemaInferenceConfig {
            ignore_outer_array: kwargs.ignore_outer_array,
            delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
            schema_uri: kwargs.schema_uri.clone(),
            map_threshold: kwargs.map_threshold,
            map_max_required_keys: kwargs.map_max_required_keys,
            unify_maps: kwargs.unify_maps,
            no_unify: kwargs.no_unify.iter().cloned().collect(),
            force_field_types: kwargs.force_field_types.clone(),
            wrap_scalars: kwargs.wrap_scalars,
            avro: kwargs.avro,
            wrap_root: wrap_root_field,
            no_root_map: kwargs.no_root_map,
            debug: kwargs.debug,
            verbosity: kwargs.verbosity,
        };

        let schema_result = infer_json_schema_from_strings(&json_strings, config)
            .map_err(|e| format!("Genson error: {}", e))?;

        let format = if kwargs.avro {
            SchemaFormat::Avro
        } else {
            SchemaFormat::JsonSchema
        };

        // Convert JSON schema to Polars field mappings
        let polars_fields = schema_to_polars_fields(&schema_result.schema, format, kwargs.debug)
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

/// Normalise a JSON string column against an inferred Avro schema.
///
/// This function performs a two-step process:
///
/// 1. **Schema inference (global):**
///    All rows in the input series are parsed as JSON and passed to
///    `genson_core::infer_json_schema_from_strings` with `avro = true`.
///    This produces a single Avro schema that captures the shape and types
///    across the entire column.
///
/// 2. **Row-wise normalisation:**
///    Each individual row is parsed again as JSON and transformed to conform
///    to the inferred schema using `normalise_values`. This ensures that
///    jagged, heterogeneous inputs (empty arrays, optional fields, differing
///    scalar/array encodings, type mismatches, etc.) are coerced into a
///    consistent representation.
///
/// The result is a new Polars Series of JSON strings, one per input row,
/// with every row guaranteed to match the same Avro schema.
///
/// # Arguments
/// * `inputs` – A slice of input Polars Series. The first must be a
///   `Utf8` (string) column containing JSON objects or arrays.
/// * `kwargs` – `GensonKwargs` struct carrying options such as:
///   - `ignore_outer_array`: Treat top-level arrays as NDJSON streams
///   - `ndjson`: Input is newline-delimited JSON
///   - `empty_as_null`: Convert empty arrays/maps to `null` (default)
///   - `coerce_string`: Allow strings to be coerced into numeric/boolean types
///   - `map_threshold`, `force_field_types`: Influence schema inference
///
/// # Returns
/// * A Polars Series of strings, length equal to the input, where each row
///   contains a JSON document normalised to the inferred Avro schema.
///
/// # Errors
/// Returns a `PolarsError::ComputeError` if the input is not a string column,
/// if schema inference fails, or if serialisation fails.
///
/// # Example
/// ```text
/// Input:
///   {"id": "1", "labels": {}}
///   {"id": 2, "labels": {"en": "Hello"}}
///
/// Normalised:
///   {"id": "1", "labels": null}
///   {"id": "2", "labels": {"en": "Hello"}}
/// ```
#[polars_expr(output_type_func=normalise_json_output_type)]
pub fn normalise_json(inputs: &[Series], kwargs: GensonKwargs) -> PolarsResult<Series> {
    if inputs.is_empty() {
        return Err(PolarsError::ComputeError("No input series provided".into()));
    }

    let series = &inputs[0];
    let string_chunked = series.str().map_err(|_| {
        PolarsError::ComputeError("Expected string column for JSON normalisation".into())
    })?;

    // Collect all JSON strings
    let mut json_strings = Vec::new();
    for s in string_chunked.iter().flatten() {
        if !s.trim().is_empty() {
            json_strings.push(s.to_string());
        }
    }

    let wrap_root_field = kwargs.wrap_root.clone();

    // Infer schema ONCE
    let config = SchemaInferenceConfig {
        ignore_outer_array: kwargs.ignore_outer_array,
        delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
        schema_uri: kwargs.schema_uri.clone(),
        map_threshold: kwargs.map_threshold,
        map_max_required_keys: kwargs.map_max_required_keys,
        unify_maps: kwargs.unify_maps,
        no_unify: kwargs.no_unify.iter().cloned().collect(),
        force_field_types: kwargs.force_field_types.clone(),
        wrap_scalars: kwargs.wrap_scalars,
        avro: true, // normalisation implies Avro
        wrap_root: wrap_root_field.clone(),
        no_root_map: kwargs.no_root_map,
        debug: kwargs.debug,
        verbosity: kwargs.verbosity,
    };

    let schema_result = infer_json_schema_from_strings(&json_strings, config)
        .map_err(|e| PolarsError::ComputeError(format!("Schema inference failed: {e}").into()))?;

    let schema = &schema_result.schema;

    // Parse each row and normalise
    let cfg = NormaliseConfig {
        empty_as_null: kwargs.empty_as_null,
        coerce_string: kwargs.coerce_string,
        map_encoding: kwargs.map_encoding,
        wrap_root: wrap_root_field.clone(),
    };

    let mut out = Vec::with_capacity(string_chunked.len());
    for s in string_chunked {
        let val = s
            .and_then(|st| serde_json::from_str::<serde_json::Value>(st).ok())
            .unwrap_or(serde_json::Value::Null);

        let normed = normalise_values(vec![val], schema, &cfg).pop().unwrap();
        out.push(serde_json::to_string(&normed).unwrap());
    }

    Ok(Series::new("normalised".into(), out))
}
