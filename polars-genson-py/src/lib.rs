use pyo3::prelude::*;

mod expressions;
mod schema;

use schema::{json_to_schema, schema_to_json};

#[pymodule]
fn _polars_genson(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(json_to_schema, m)?)?;
    m.add_function(wrap_pyfunction!(schema_to_json, m)?)?;
    Ok(())
}

// Note: We don't set up a PolarsAllocator here because genson_rs already
// defines a global allocator, and Rust only allows one global allocator per binary.
// The existing allocator from genson_rs is sufficient for our needs.
