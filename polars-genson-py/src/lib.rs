use pyo3::prelude::*;

mod expressions;

#[pymodule]
fn _polars_genson(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}