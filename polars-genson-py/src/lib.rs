use pyo3::prelude::*;

mod expressions;
mod parquet_io;
mod schema;

use parquet_io::{infer_from_parquet, normalise_from_parquet, read_parquet_metadata};
use schema::{json_to_schema, schema_to_json};

#[pyfunction]
fn avro_to_polars_fields(schema_json: String, debug: bool) -> PyResult<Vec<(String, String)>> {
    use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};

    let schema: serde_json::Value = serde_json::from_str(&schema_json)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON: {}", e)))?;

    let fields = schema_to_polars_fields(&schema, SchemaFormat::Avro, debug).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Conversion failed: {}", e))
    })?;

    Ok(fields)
}

#[pymodule]
fn _polars_genson(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(json_to_schema, m)?)?;
    m.add_function(wrap_pyfunction!(schema_to_json, m)?)?;
    m.add_function(wrap_pyfunction!(infer_from_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(normalise_from_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(read_parquet_metadata, m)?)?;
    m.add_function(wrap_pyfunction!(avro_to_polars_fields, m)?)?;
    Ok(())
}

// Note: We don't set up a PolarsAllocator here because genson_rs already
// defines a global allocator, and Rust only allows one global allocator per binary.
// The existing allocator from genson_rs is sufficient for our needs.
