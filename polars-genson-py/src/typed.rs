//! `normalise_json` decoded to Polars dtypes in one pass: the schema inferred once, and the
//! rows normalised straight into Arrow by the typed decoder (`ArrowPlan`), instead of
//! normalised to JSON text that `str.json_decode` parses again against a schema inferred a
//! second time.
use crate::expressions::GensonKwargs;
use arrow::array::{Array, StructArray};
use genson_core::normalise::{NormaliseConfig, NormalisePlan};
use genson_core::parquet::{avro_record_fields, values_to_struct_array};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::PySeries;
use rayon::prelude::*;
use std::collections::HashSet;

fn err(e: impl std::fmt::Display) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(e.to_string())
}

/// The column normalised and decoded into a struct Series, with the Avro schema it was
/// normalised against (for the caller to give it the Polars dtypes that schema converts to).
/// `None` where the typed decoder does not apply (the root is not a record, or the map
/// encoding is `entries`), or inference or decoding fails, for the caller to take the JSON
/// text route, which raises its errors (or decodes the rows) as before.
#[pyfunction]
pub fn normalise_json_typed(
    series: PySeries,
    kwargs_json: &str,
) -> PyResult<Option<(PySeries, String)>> {
    let kwargs: GensonKwargs = serde_json::from_str(kwargs_json).map_err(err)?;
    let series: Series = series.into();
    let ca = series.str().map_err(err)?;
    let rows: Vec<Option<&str>> = ca.iter().collect();
    let native_map = match kwargs.map_encoding {
        genson_core::normalise::MapEncoding::Mapping => true,
        genson_core::normalise::MapEncoding::KeyValueEntries => false,
        genson_core::normalise::MapEncoding::Entries => return Ok(None),
    };

    // Inferred as `normalise_json` infers it
    let json_strings: Vec<&str> = rows
        .iter()
        .flatten()
        .copied()
        .filter(|s| !s.trim().is_empty())
        .collect();
    let config = SchemaInferenceConfig {
        ignore_outer_array: kwargs.ignore_outer_array,
        delimiter: if kwargs.ndjson { Some(b'\n') } else { None },
        schema_uri: kwargs.schema_uri.clone(),
        map_threshold: kwargs.map_threshold,
        map_max_required_keys: kwargs.map_max_required_keys,
        unify_maps: kwargs.unify_maps,
        no_unify: kwargs.no_unify.iter().cloned().collect(),
        force_field_types: kwargs.force_field_types.clone(),
        force_parent_field_types: kwargs.force_parent_field_types.clone(),
        force_scalar_promotion: kwargs.force_scalar_promotion.iter().cloned().collect(),
        wrap_scalars: kwargs.wrap_scalars,
        coerce_strings: kwargs.coerce_string,
        avro: true,
        wrap_root: kwargs.wrap_root.clone(),
        no_root_map: kwargs.no_root_map,
        max_builders: kwargs.max_builders,
        debug: kwargs.debug,
        profile: kwargs.profile,
        verbosity: kwargs.verbosity,
    };
    let Ok(result) = infer_json_schema_from_strings(&json_strings, config) else {
        return Ok(None);
    };
    let schema = result.schema;
    drop(json_strings);
    if schema.get("type").and_then(|t| t.as_str()) != Some("record") {
        return Ok(None);
    }
    let Ok(fields) = avro_record_fields(&schema, native_map) else {
        return Ok(None);
    };

    let cfg = NormaliseConfig {
        empty_as_null: kwargs.empty_as_null,
        coerce_string: kwargs.coerce_string,
        map_encoding: kwargs.map_encoding,
        wrap_root: kwargs.wrap_root.clone(),
    };
    let plan = NormalisePlan::new(&schema, &cfg, &HashSet::new());
    let arrow_plan = plan.to_arrow(&fields);
    // As `normalise_from_parquet`'s typed output: batches straight from the rows' text
    // where the decoder can do that exactly, else by way of `Value`s
    let batch_rows = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let arrays = rows
        .par_chunks(batch_rows)
        .enumerate()
        .map(|(b, chunk)| {
            if let Some((array, _)) = arrow_plan
                .as_ref()
                .and_then(|p| p.normalise(chunk, b * batch_rows))
            {
                return Ok(array);
            }
            let values: Vec<serde_json::Value> = chunk
                .iter()
                .map(|s| {
                    let v = s
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(serde_json::Value::Null);
                    plan.normalise(v).0
                })
                .collect();
            values_to_struct_array(&values, &fields)
        })
        .collect::<Result<Vec<StructArray>, String>>();
    // A batch the decoder rejects is left to the JSON text route, which decodes it its own way
    let Ok(arrays) = arrays else {
        return Ok(None);
    };
    let parts: Vec<&dyn Array> = arrays.iter().map(|a| a as &dyn Array).collect();
    let array = if parts.is_empty() {
        StructArray::new_null(fields.clone(), 0)
    } else {
        arrow::compute::concat(&parts)
            .map_err(err)?
            .as_any()
            .downcast_ref::<StructArray>()
            .expect("struct arrays concatenate to a struct array")
            .clone()
    };
    let typed = to_series(
        series.name().clone(),
        fields_null_where_row_is(array)?.into_data(),
    )?;
    Ok(Some((PySeries(typed), schema.to_string())))
}

/// A null row (a missing or unparseable input) as a struct whose fields are all null, which
/// is what `str.json_decode` makes of the JSON text route's `null`: the row's nulls moved
/// into its fields.
fn fields_null_where_row_is(array: StructArray) -> PyResult<StructArray> {
    let Some(rows) = array.nulls().cloned() else {
        return Ok(array);
    };
    let (fields, columns, _) = array.into_parts();
    let columns = columns
        .into_iter()
        .map(|c| {
            let nulls = arrow::buffer::NullBuffer::union(Some(&rows), c.nulls());
            let data = c
                .to_data()
                .into_builder()
                .nulls(nulls)
                .build()
                .map_err(err)?;
            Ok(arrow::array::make_array(data))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let fields: arrow::datatypes::Fields = fields
        .iter()
        .map(|f| f.as_ref().clone().with_nullable(true))
        .collect();
    StructArray::try_new(fields, columns, None).map_err(err)
}

/// An arrow-rs array as a Polars Series, by way of the Arrow C data interface.
fn to_series(name: PlSmallStr, data: arrow::array::ArrayData) -> PyResult<Series> {
    let (array, schema) = arrow::ffi::to_ffi(&data).map_err(err)?;
    // The two crates' C structs are the C data interface's, field for field; ownership
    // (and the release callbacks) moves to Polars
    unsafe {
        let schema: polars_arrow::ffi::ArrowSchema = std::mem::transmute(schema);
        let array: polars_arrow::ffi::ArrowArray = std::mem::transmute(array);
        let field = polars_arrow::ffi::import_field_from_c(&schema).map_err(err)?;
        let imported = polars_arrow::ffi::import_array_from_c(array, field.dtype).map_err(err)?;
        Series::from_arrow(name, imported).map_err(err)
    }
}
