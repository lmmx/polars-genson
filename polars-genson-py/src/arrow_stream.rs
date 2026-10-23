//! Record batches handed to Python through the Arrow PyCapsule interface, without a file.

use std::sync::Mutex;

use arrow::datatypes::{DataType, Fields, Schema, SchemaRef};
use arrow::ffi::FFI_ArrowSchema;
use arrow::ffi_stream::FFI_ArrowArrayStream;
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyCapsule;

/// An Arrow C stream of record batches, read once by any reader of the Arrow PyCapsule
/// interface (`pyarrow.RecordBatchReader.from_stream`, `pyarrow.table`). The batches'
/// buffers are moved to the reader, not copied; a stream that is never read releases
/// them when it is dropped.
#[pyclass(module = "polars_genson", name = "ArrowStream")]
pub struct ArrowStream {
    schema: SchemaRef,
    stream: Mutex<Option<FFI_ArrowArrayStream>>,
}

impl ArrowStream {
    pub fn new(schema: SchemaRef, batches: Vec<RecordBatch>) -> Self {
        let reader = RecordBatchIterator::new(batches.into_iter().map(Ok), schema.clone());
        Self {
            schema,
            stream: Mutex::new(Some(FFI_ArrowArrayStream::new(Box::new(reader)))),
        }
    }
}

#[pymethods]
impl ArrowStream {
    /// The stream as a PyCapsule named `arrow_array_stream`.
    ///
    /// A `requested_schema` (a PyCapsule named `arrow_schema`) is checked against the
    /// stream's schema. The batches are not cast, so they come in their own schema when
    /// it holds the same data in another representation (the same fields, by name and
    /// order, with `large_string` for `string`, a list type for another, or a dictionary
    /// for its values), as the interface allows. A request for other data raises
    /// `ValueError`, and leaves the stream to be read.
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        if let Some(requested) = requested_schema.filter(|r| !r.is_none()) {
            let requested = schema_from_capsule(&requested)?;
            if let Some(why) = incompatibility(self.schema.fields(), requested.fields()) {
                return Err(PyValueError::new_err(format!(
                    "The Arrow stream cannot be given in the requested schema: {why}"
                )));
            }
        }
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

/// The schema in a PyCapsule named `arrow_schema`, borrowed: the capsule keeps it and
/// releases it.
fn schema_from_capsule(requested: &Bound<'_, PyAny>) -> PyResult<Schema> {
    let capsule = requested.cast::<PyCapsule>().map_err(|_| {
        PyTypeError::new_err("requested_schema must be a PyCapsule named 'arrow_schema'")
    })?;
    let pointer = capsule.pointer_checked(Some(c"arrow_schema"))?;
    // SAFETY: a capsule named `arrow_schema` holds an FFI_ArrowSchema (the Arrow PyCapsule
    // interface), alive while the capsule is; it is only read here, not moved or released
    let ffi = unsafe { pointer.cast::<FFI_ArrowSchema>().as_ref() };
    Schema::try_from(ffi)
        .map_err(|e| PyValueError::new_err(format!("Invalid requested_schema: {e}")))
}

/// Why `requested` does not describe the same data as `own`, if it does not: other field
/// names or another order, or a field of another type than a representation of its own.
fn incompatibility(own: &Fields, requested: &Fields) -> Option<String> {
    let names = |f: &Fields| f.iter().map(|f| f.name().clone()).collect::<Vec<_>>();
    if names(own) != names(requested) {
        return Some(format!(
            "fields {:?}, not {:?}",
            names(requested),
            names(own)
        ));
    }
    own.iter().zip(requested.iter()).find_map(|(a, b)| {
        (!same_data(a.data_type(), b.data_type())).then(|| {
            format!(
                "field '{}' as {}, not {}",
                a.name(),
                b.data_type(),
                a.data_type()
            )
        })
    })
}

/// Whether two types hold the same values, as different representations at most: string,
/// binary and list types of other offset widths or views, or a dictionary of the values.
fn same_data(a: &DataType, b: &DataType) -> bool {
    use DataType::*;
    match (a, b) {
        (Dictionary(_, a), b) | (b, Dictionary(_, a)) => same_data(a, b),
        (Utf8 | LargeUtf8 | Utf8View, Utf8 | LargeUtf8 | Utf8View) => true,
        (Binary | LargeBinary | BinaryView, Binary | LargeBinary | BinaryView) => true,
        (
            List(a) | LargeList(a) | ListView(a) | LargeListView(a),
            List(b) | LargeList(b) | ListView(b) | LargeListView(b),
        ) => same_data(a.data_type(), b.data_type()),
        (Struct(a), Struct(b)) => incompatibility(a, b).is_none(),
        _ => a == b,
    }
}
