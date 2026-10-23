//! Record batches handed to Python through the Arrow PyCapsule interface, without a file.

use std::sync::Mutex;

use arrow::datatypes::SchemaRef;
use arrow::ffi_stream::FFI_ArrowArrayStream;
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyCapsule;

/// An Arrow C stream of record batches, read once by any reader of the Arrow PyCapsule
/// interface (`pyarrow.RecordBatchReader.from_stream`, `pyarrow.table`). The batches'
/// buffers are moved to the reader, not copied; a stream that is never read releases
/// them when it is dropped.
#[pyclass(module = "polars_genson", name = "ArrowStream")]
pub struct ArrowStream {
    stream: Mutex<Option<FFI_ArrowArrayStream>>,
}

impl ArrowStream {
    pub fn new(schema: SchemaRef, batches: Vec<RecordBatch>) -> Self {
        let reader = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
        Self {
            stream: Mutex::new(Some(FFI_ArrowArrayStream::new(Box::new(reader)))),
        }
    }
}

#[pymethods]
impl ArrowStream {
    /// The stream as a PyCapsule named `arrow_array_stream`. The batches are given in
    /// their own schema, whatever `requested_schema` asks for, as the interface allows.
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        let stream = self
            .stream
            .lock()
            .map_err(|_| PyValueError::new_err("The Arrow stream's lock is poisoned"))?
            .take()
            .ok_or_else(|| PyValueError::new_err("The Arrow stream has already been read"))?;
        // A reader moves the stream out, leaving it released; the destructor drops what
        // is left, which releases a stream no reader took
        PyCapsule::new_with_value_and_destructor(py, stream, c"arrow_array_stream", |s, _| drop(s))
    }
}
