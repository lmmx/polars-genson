use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::types::PyDataFrame;
use serde_json::{Map, Value};

/// Prints to stderr in orange, with formatting support.
#[macro_export]
macro_rules! eprintln_orange {
    ($($arg:tt)*) => {{
        const ORANGE: &str = "\x1b[38;5;202m";
        const RESET: &str = "\x1b[0m";
        eprintln!("{ORANGE}(🦀) {}{}", format!($($arg)*), RESET);
    }};
}

/// Convert a Polars schema to JSON string representation
#[pyfunction]
pub fn schema_to_json(df: PyDataFrame) -> PyResult<String> {
    eprintln_orange!("Got df: {:?}", df);

    let schema = df.0.schema();

    let mut schema_map = Map::new();
    for (name, dtype) in schema.iter() {
        let dtype_json = serde_json::to_value(dtype).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize dtype: {}", e))
        })?;

        eprintln_orange!(
            "field: {}, dtype: {:?}, as JSON: {}",
            name,
            dtype,
            dtype_json
        );

        schema_map.insert(name.to_string(), dtype_json);
    }

    let out = serde_json::to_string_pretty(&schema_map).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to serialize schema to JSON: {}",
            e
        ))
    })?;

    eprintln_orange!("Final schema JSON:\n{}", out);

    Ok(out)
}

#[pyfunction]
pub fn json_to_schema(json_str: &str) -> PyResult<PyDataFrame> {
    eprintln_orange!("Loading schema JSON:\n{}", json_str);

    let schema_map: Map<String, Value> = serde_json::from_str(json_str).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Failed to parse schema JSON: {}", e))
    })?;

    let schema: Schema = schema_map
        .into_iter()
        .map(|(name, dtype_val)| -> PyResult<(PlSmallStr, DataType)> {
            eprintln_orange!("deserializing field: {}, raw JSON: {}", name, dtype_val);

            let dtype: DataType = serde_json::from_value(dtype_val).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "Failed to deserialize dtype for {}: {}",
                    name, e
                ))
            })?;

            Ok((name.into(), dtype))
        })
        .collect::<Result<Schema, _>>()?;

    eprintln_orange!("Reconstructed schema: {:?}", schema);

    // 🔑 Make an empty DataFrame with the schema
    let df = DataFrame::empty_with_schema(&schema);

    Ok(PyDataFrame(df))
}
