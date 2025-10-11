use genson_core::normalise::{normalise_values, MapEncoding, NormaliseConfig};
use genson_core::parquet::{read_string_column, write_string_column};
use genson_core::{infer_json_schema_from_strings, DebugVerbosity, SchemaInferenceConfig};
use pyo3::prelude::*;
use std::collections::HashMap;

#[pyfunction]
#[pyo3(signature = (
    input_path,
    column,
    output_path=None,
    ignore_outer_array=true,
    ndjson=false,
    schema_uri=Some("http://json-schema.org/schema#".to_string()),
    debug=false,
    profile=false,
    verbosity="Normal".to_string(),
    map_threshold=20,
    map_max_required_keys=None,
    unify_maps=false,
    no_unify=None,
    force_field_types=None,
    force_parent_field_types=None,
    force_scalar_promotion=None,
    wrap_scalars=true,
    avro=false,
    wrap_root=None,
    no_root_map=true,
    max_builders=None,
))]
#[allow(clippy::too_many_arguments)]
pub fn infer_from_parquet(
    input_path: String,
    column: String,
    output_path: Option<String>,
    ignore_outer_array: bool,
    ndjson: bool,
    schema_uri: Option<String>,
    debug: bool,
    profile: bool,
    verbosity: String,
    map_threshold: usize,
    map_max_required_keys: Option<usize>,
    unify_maps: bool,
    no_unify: Option<Vec<String>>,
    force_field_types: Option<HashMap<String, String>>,
    force_parent_field_types: Option<HashMap<String, String>>,
    force_scalar_promotion: Option<Vec<String>>,
    wrap_scalars: bool,
    avro: bool,
    wrap_root: Option<String>,
    no_root_map: bool,
    max_builders: Option<usize>,
) -> PyResult<String> {
    // Read from Parquet
    let json_strings = read_string_column(&input_path, &column).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read Parquet: {}", e))
    })?;

    if debug {
        anstream::eprintln!(
            "Read {} JSON strings from column '{}'",
            json_strings.len(),
            column
        );
    }

    // Parse verbosity
    let verbosity_enum = match verbosity.as_str() {
        "Verbose" => DebugVerbosity::Verbose,
        _ => DebugVerbosity::Normal,
    };

    // Build config
    let config = SchemaInferenceConfig {
        ignore_outer_array,
        delimiter: if ndjson { Some(b'\n') } else { None },
        schema_uri,
        map_threshold,
        map_max_required_keys,
        unify_maps,
        no_unify: no_unify.unwrap_or_default().into_iter().collect(),
        force_field_types: force_field_types.unwrap_or_default(),
        force_parent_field_types: force_parent_field_types.unwrap_or_default(),
        force_scalar_promotion: force_scalar_promotion
            .unwrap_or_default()
            .into_iter()
            .collect(),
        wrap_scalars,
        avro,
        wrap_root,
        no_root_map,
        max_builders,
        debug,
        profile,
        verbosity: verbosity_enum,
    };

    // Infer schema
    let result = infer_json_schema_from_strings(&json_strings, config).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Schema inference failed: {}", e))
    })?;

    // Serialize schema
    let schema_json = serde_json::to_string_pretty(&result.schema).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("JSON serialization failed: {}", e))
    })?;

    // Print to stderr
    anstream::eprintln!("Processed {} JSON object(s)", result.processed_count);

    // Write to file or return
    if let Some(out_path) = output_path {
        std::fs::write(&out_path, &schema_json).map_err(|e| {
            pyo3::exceptions::PyIOError::new_err(format!("Failed to write to {}: {}", out_path, e))
        })?;
        anstream::eprintln!("Schema written to: {}", out_path);
        Ok(format!("Schema written to: {}", out_path))
    } else {
        Ok(schema_json)
    }
}

#[pyfunction]
#[pyo3(signature = (
    input_path,
    column,
    output_path,
    output_column=None,
    ignore_outer_array=true,
    ndjson=false,
    empty_as_null=true,
    coerce_strings=false,
    map_encoding="kv".to_string(),
    profile=false,
    map_threshold=20,
    map_max_required_keys=None,
    unify_maps=false,
    no_unify=None,
    force_field_types=None,
    force_parent_field_types=None,
    force_scalar_promotion=None,
    wrap_scalars=true,
    wrap_root=None,
    no_root_map=true,
    max_builders=None,
))]
#[allow(clippy::too_many_arguments)]
pub fn normalise_from_parquet(
    input_path: String,
    column: String,
    output_path: String,
    output_column: Option<String>,
    ignore_outer_array: bool,
    ndjson: bool,
    empty_as_null: bool,
    coerce_strings: bool,
    map_encoding: String,
    profile: bool,
    map_threshold: usize,
    map_max_required_keys: Option<usize>,
    unify_maps: bool,
    no_unify: Option<Vec<String>>,
    force_field_types: Option<HashMap<String, String>>,
    force_parent_field_types: Option<HashMap<String, String>>,
    force_scalar_promotion: Option<Vec<String>>,
    wrap_scalars: bool,
    wrap_root: Option<String>,
    no_root_map: bool,
    max_builders: Option<usize>,
) -> PyResult<()> {
    // Read from Parquet
    let json_strings = read_string_column(&input_path, &column).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read Parquet: {}", e))
    })?;

    anstream::eprintln!(
        "Read {} JSON strings from column '{}'",
        json_strings.len(),
        column
    );

    // Infer schema first (Avro mode)
    let config = SchemaInferenceConfig {
        ignore_outer_array,
        delimiter: if ndjson { Some(b'\n') } else { None },
        schema_uri: None,
        map_threshold,
        map_max_required_keys,
        unify_maps,
        no_unify: no_unify.unwrap_or_default().into_iter().collect(),
        force_field_types: force_field_types.unwrap_or_default(),
        force_parent_field_types: force_parent_field_types.unwrap_or_default(),
        force_scalar_promotion: force_scalar_promotion
            .unwrap_or_default()
            .into_iter()
            .collect(),
        wrap_scalars,
        avro: true,
        wrap_root: wrap_root.clone(),
        no_root_map,
        max_builders,
        debug: false,
        profile,
        verbosity: DebugVerbosity::Normal,
    };

    let result = infer_json_schema_from_strings(&json_strings, config).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Schema inference failed: {}", e))
    })?;

    anstream::eprintln!("Processed {} JSON object(s)", result.processed_count);

    // Parse values
    let values: Vec<serde_json::Value> = json_strings
        .iter()
        .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::Null))
        .collect();

    // Parse map encoding
    let map_enc = match map_encoding.as_str() {
        "mapping" => MapEncoding::Mapping,
        "entries" => MapEncoding::Entries,
        "kv" => MapEncoding::KeyValueEntries,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Invalid map_encoding: {}",
                map_encoding
            )))
        }
    };

    // Normalise
    let norm_config = NormaliseConfig {
        empty_as_null,
        coerce_string: coerce_strings,
        map_encoding: map_enc,
        wrap_root: wrap_root.clone(),
    };

    let normalised = normalise_values(values, &result.schema, &norm_config);

    // Convert back to JSON strings
    let normalised_strings: Vec<String> = normalised
        .into_iter()
        .map(|v| serde_json::to_string(&v).unwrap())
        .collect();

    // Always write to Parquet
    let col_name = output_column.unwrap_or_else(|| column.clone());

    let mut metadata = HashMap::new();
    metadata.insert(
        "genson_avro_schema".to_string(),
        serde_json::to_string(&result.schema).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize schema: {}", e))
        })?,
    );
    metadata.insert(
        "genson_normalise_config".to_string(),
        serde_json::to_string(&norm_config).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize config: {}", e))
        })?,
    );

    write_string_column(&output_path, &col_name, normalised_strings, Some(metadata)).map_err(
        |e| pyo3::exceptions::PyIOError::new_err(format!("Failed to write Parquet: {}", e)),
    )?;

    anstream::eprintln!(
        "Normalised data written to: {} (column: {})",
        output_path,
        col_name
    );

    Ok(())
}

#[pyfunction]
pub fn read_parquet_metadata(path: String) -> PyResult<HashMap<String, String>> {
    genson_core::parquet::read_parquet_metadata(&path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read metadata: {}", e))
    })
}
