use arrow::datatypes::DataType;
use genson_core::conform::{conform, data_type_from_spec, incompatibilities, SchemaCheck};
use genson_core::normalise::{
    prune_schema, MapEncoding, NormaliseConfig, NormalisePlan, PrunedValue,
};
use genson_core::parquet::{
    avro_record_fields, read_columns, read_string_column_arrays, values_to_struct_array,
    write_string_column_with, write_struct_columns, StructLayout,
};
use genson_core::{infer_json_schema_from_strings, DebugVerbosity, SchemaInferenceConfig};
use pyo3::prelude::*;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

fn value_err(e: impl std::fmt::Display) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(e.to_string())
}

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
    // Read from Parquet: the column's rows, borrowed from the decoded arrays (nulls skipped)
    let column_data = read_string_column_arrays(&input_path, &column).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read Parquet: {}", e))
    })?;
    let json_strings: Vec<&str> = column_data.rows().into_iter().flatten().collect();

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
        coerce_strings: false,
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

    if debug {
        anstream::eprintln!("Processed {} JSON object(s)", result.processed_count);
    }

    // Write to file or return
    if let Some(out_path) = output_path {
        std::fs::write(&out_path, &schema_json).map_err(|e| {
            pyo3::exceptions::PyIOError::new_err(format!("Failed to write to {}: {}", out_path, e))
        })?;
        if debug {
            anstream::eprintln!("Schema written to: {}", out_path);
        }
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
    debug=false,
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
    typed=false,
    keep_columns=None,
    extract_invariants=None,
    lookup_output_path=None,
    prune=None,
    prune_output_path=None,
    schema=None,
    schema_check="compatible".to_string(),
    unnest=false,
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
    debug: bool,
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
    typed: bool,
    keep_columns: Option<Vec<String>>,
    extract_invariants: Option<Vec<(String, String)>>,
    lookup_output_path: Option<String>,
    prune: Option<Vec<String>>,
    prune_output_path: Option<String>,
    schema: Option<String>,
    schema_check: String,
    unnest: bool,
) -> PyResult<()> {
    if (schema.is_some() || unnest) && !typed {
        return Err(value_err(
            "schema and unnest need typed output (typed=True)",
        ));
    }
    let check: SchemaCheck = schema_check.parse().map_err(value_err)?;
    // The output column's target type, a struct (see genson_core::conform)
    let target = match &schema {
        Some(spec) => {
            let spec: serde_json::Value = serde_json::from_str(spec)
                .map_err(|e| value_err(format!("Invalid target schema: {e}")))?;
            match data_type_from_spec(&spec).map_err(value_err)? {
                t @ DataType::Struct(_) => Some(t),
                other => {
                    return Err(value_err(format!(
                        "The target schema must be a struct, not {other}"
                    )))
                }
            }
        }
        None => None,
    };
    if typed && !matches!(map_encoding.as_str(), "kv" | "mapping") {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "typed output requires map_encoding=\"kv\" or \"mapping\"",
        ));
    }
    let mut t0 = std::time::Instant::now();
    let tick = |what: &str, t: &mut std::time::Instant| {
        if profile {
            anstream::eprintln!("[profile] {}: {:.3}s", what, t.elapsed().as_secs_f64());
            *t = std::time::Instant::now();
        }
    };
    if extract_invariants.is_some() != lookup_output_path.is_some() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "extract_invariants and lookup_output_path must be given together",
        ));
    }
    if prune.is_some() != prune_output_path.is_some() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "prune and prune_output_path must be given together",
        ));
    }
    let prune: HashSet<String> = prune.unwrap_or_default().into_iter().collect();
    // Read from Parquet, one entry per row so null rows stay aligned with `keep`; the rows
    // are borrowed from the decoded arrays
    let column_data = read_string_column_arrays(&input_path, &column).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read Parquet: {}", e))
    })?;
    let keep = read_columns(&input_path, &keep_columns.unwrap_or_default()).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read Parquet: {}", e))
    })?;

    if debug {
        anstream::eprintln!(
            "Read {} JSON strings from column '{}'",
            column_data.len(),
            column
        );
    }

    tick("read_parquet", &mut t0);
    // Move the extract_invariants fields out of the rows before inference sees them
    let slim_rows = match (&extract_invariants, &lookup_output_path) {
        (Some(spec), Some(path)) => {
            let extracted =
                genson_core::extract::extract_invariants_from(&column_data.rows(), spec)
                    .map_err(pyo3::exceptions::PyValueError::new_err)?;
            tick("extract_invariants", &mut t0);
            Some((extracted.rows, extracted.lookup, path.clone()))
        }
        _ => None,
    };
    // Written once the inferred schema has passed its check, so that nothing is written
    // for a chunk whose schema is rejected
    let (slim_rows, lookup) = match slim_rows {
        Some((rows, lookup, path)) => (Some(rows), Some((lookup, path))),
        None => (None, None),
    };
    let write_lookup =
        |lookup: Option<(Vec<genson_core::extract::LookupEntry>, String)>| -> PyResult<()> {
            match lookup {
                Some((table, path)) => genson_core::parquet::write_lookup_table(&path, &table)
                    .map_err(|e| {
                        pyo3::exceptions::PyIOError::new_err(format!(
                            "Failed to write lookup table: {}",
                            e
                        ))
                    }),
                None => Ok(()),
            }
        };
    // The slim rows replace the read ones, which are then no longer needed
    let column_data = slim_rows.is_none().then_some(column_data);
    let json_strings: Vec<Option<&str>> = match (&slim_rows, &column_data) {
        (Some(rows), _) => rows.iter().map(Option::as_deref).collect(),
        (None, Some(column_data)) => column_data.rows(),
        (None, None) => unreachable!("the read rows are kept when nothing was extracted"),
    };
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
        coerce_strings,
        avro: true,
        wrap_root: wrap_root.clone(),
        no_root_map,
        max_builders,
        debug,
        profile,
        verbosity: DebugVerbosity::Normal,
    };

    let present: Vec<&str> = json_strings.iter().flatten().copied().collect();
    let result = infer_json_schema_from_strings(&present, config).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Schema inference failed: {}", e))
    })?;
    drop(present);
    // Rows are normalised against the inferred schema (prune names are matched against its
    // fields) and written with the pruned one
    let mut out_schema = result.schema.clone();
    prune_schema(&mut out_schema, &prune);

    if debug {
        anstream::eprintln!("Processed {} JSON object(s)", result.processed_count);
    }

    tick("infer", &mut t0);

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

    let norm_config = NormaliseConfig {
        empty_as_null,
        coerce_string: coerce_strings,
        map_encoding: map_enc,
        wrap_root: wrap_root.clone(),
    };

    // Normalise one row, numbering its pruned values with the row's index; the schema is
    // compiled once for all the rows
    let plan = NormalisePlan::new(&result.schema, &norm_config, &prune);
    let normalise_row = |row: usize, s: &Option<&str>| -> (serde_json::Value, Vec<PrunedValue>) {
        let Some(s) = s else {
            return (serde_json::Value::Null, Vec::new());
        };
        let v = serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
        let (normalised, mut pruned) = plan.normalise(v);
        pruned.iter_mut().for_each(|p| p.row = row);
        (normalised, pruned)
    };
    let write_pruned = |pruned: Vec<PrunedValue>| -> PyResult<()> {
        let Some(path) = &prune_output_path else {
            return Ok(());
        };
        let rows: Vec<usize> = pruned.iter().map(|p| p.row).collect();
        let (paths, values) = pruned
            .iter()
            .map(|p| {
                (
                    Some(serde_json::to_string(&p.path).unwrap()),
                    Some(p.value.to_string()),
                )
            })
            .unzip();
        genson_core::parquet::write_pruned_table(path, &rows, paths, values, &keep).map_err(|e| {
            pyo3::exceptions::PyIOError::new_err(format!("Failed to write pruned table: {}", e))
        })
    };

    let col_name = output_column.unwrap_or_else(|| column.clone());
    let mut metadata = HashMap::new();
    metadata.insert(
        "genson_avro_schema".to_string(),
        serde_json::to_string(&out_schema).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize schema: {}", e))
        })?,
    );
    metadata.insert(
        "genson_normalise_config".to_string(),
        serde_json::to_string(&norm_config).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize config: {}", e))
        })?,
    );

    if typed {
        let fields = avro_record_fields(&out_schema, map_encoding == "mapping")
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        if let Some(target) = &target {
            let problems = incompatibilities(&DataType::Struct(fields.clone()), target, check);
            if !problems.is_empty() {
                return Err(value_err(format!(
                    "The inferred schema of '{}' does not fit the target schema (schema_check=\"{}\"):\n  {}",
                    column,
                    schema_check,
                    problems.join("\n  ")
                )));
            }
        }
        write_lookup(lookup)?;
        // Rows go straight from their text into Arrow where the plan can do that exactly,
        // else (a batch at a time) by way of `Value`s
        let arrow_plan = plan.to_arrow(&fields);
        // Several batches per thread so uneven row sizes still spread over the pool
        let batch_rows = json_strings
            .len()
            .div_ceil(rayon::current_num_threads() * 4)
            .max(1);
        let batches = json_strings
            .par_chunks(batch_rows)
            .enumerate()
            .map(|(b, chunk)| {
                let decoded = arrow_plan
                    .as_ref()
                    .and_then(|p| p.normalise(chunk, b * batch_rows));
                if let Some((array, pruned)) = decoded {
                    return Ok((array, vec![pruned]));
                }
                let (rows, pruned): (Vec<_>, Vec<_>) = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, s)| normalise_row(b * batch_rows + i, s))
                    .unzip();
                values_to_struct_array(&rows, &fields).map(|a| (a, pruned))
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(pyo3::exceptions::PyValueError::new_err)?;
        let (arrays, pruned): (Vec<_>, Vec<_>) = batches.into_iter().unzip();
        drop(json_strings);
        tick("parse+normalise+decode", &mut t0);
        // In the target's shape: its fields in its order, those the rows lacked as nulls
        let (arrays, fields) = match &target {
            Some(target) => {
                let arrays = arrays
                    .into_par_iter()
                    .map(|a| {
                        let a: arrow::array::ArrayRef = std::sync::Arc::new(a);
                        conform(&a, target).map(|c| {
                            c.as_any()
                                .downcast_ref::<arrow::array::StructArray>()
                                .expect("a struct conforms to a struct")
                                .clone()
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
                    .map_err(value_err)?;
                let DataType::Struct(fields) = target else {
                    unreachable!("checked above")
                };
                tick("conform", &mut t0);
                (arrays, fields.clone())
            }
            None => (arrays, fields),
        };
        write_pruned(pruned.into_iter().flatten().flatten().collect())?;
        write_struct_columns(
            &output_path,
            &col_name,
            arrays,
            &fields,
            &keep,
            Some(metadata),
            // Laid out as Polars writes the target schema (see `data_type_from_spec`)
            StructLayout {
                unnest,
                spec_names: target.is_some(),
            },
        )
        .map_err(|e| {
            pyo3::exceptions::PyIOError::new_err(format!("Failed to write Parquet: {}", e))
        })?;
        tick("write_parquet", &mut t0);
        return Ok(());
    }

    // Parse, normalise and re-serialise each row in parallel (order-preserving)
    let (normalised_strings, pruned): (Vec<Option<String>>, Vec<Vec<PrunedValue>>) = json_strings
        .par_iter()
        .enumerate()
        .map(|(row, s)| {
            let (n, pruned) = normalise_row(row, s);
            (
                s.as_ref().map(|_| serde_json::to_string(&n).unwrap()),
                pruned,
            )
        })
        .unzip();
    drop(json_strings);
    tick("parse+normalise+serialise", &mut t0);
    write_lookup(lookup)?;
    write_pruned(pruned.into_iter().flatten().collect())?;

    write_string_column_with(
        &output_path,
        &col_name,
        normalised_strings,
        &keep,
        Some(metadata),
    )
    .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Failed to write Parquet: {}", e)))?;

    tick("write_parquet", &mut t0);
    if debug {
        anstream::eprintln!(
            "Normalised data written to: {} (column: {})",
            output_path,
            col_name
        );
    }

    Ok(())
}

#[pyfunction]
pub fn read_parquet_metadata(path: String) -> PyResult<HashMap<String, String>> {
    genson_core::parquet::read_parquet_metadata(&path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to read metadata: {}", e))
    })
}

/// Where `inferred` does not pass `schema_check` against `target`, both dtypes described as
/// JSON by the Python package (see `genson_core::conform::data_type_from_spec`): one line
/// per offending field, its path and why; empty when it passes.
#[pyfunction]
#[pyo3(signature = (inferred, target, schema_check="compatible".to_string()))]
pub fn schema_incompatibilities(
    inferred: String,
    target: String,
    schema_check: String,
) -> PyResult<Vec<String>> {
    let check: SchemaCheck = schema_check.parse().map_err(value_err)?;
    let parse = |s: &str| -> PyResult<DataType> {
        let spec: serde_json::Value =
            serde_json::from_str(s).map_err(|e| value_err(format!("Invalid schema: {e}")))?;
        data_type_from_spec(&spec).map_err(value_err)
    };
    Ok(incompatibilities(
        &parse(&inferred)?,
        &parse(&target)?,
        check,
    ))
}
