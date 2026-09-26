//! Parquet file I/O for reading and writing string columns

use arrow::array::{Array, ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, FieldRef, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::{ArrowWriter, ProjectionMask};
use parquet::file::properties::WriterProperties;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;

/// Read a string column from a Parquet file
///
/// # Arguments
/// * `path` - Path to the Parquet file
/// * `column_name` - Name of the string column to extract
///
/// # Returns
/// Vector of strings from the specified column (nulls are skipped)
///
/// # Errors
/// Returns error if:
/// - File cannot be opened
/// - Column doesn't exist
/// - Column is not a string type (Utf8 or LargeUtf8)
pub fn read_string_column(path: &str, column_name: &str) -> Result<Vec<String>, String> {
    Ok(read_string_column_opt(path, column_name)?
        .into_iter()
        .flatten()
        .collect())
}

/// Read a string column from a Parquet file, one entry per row (`None` for null rows).
///
/// # Errors
/// As for [`read_string_column`].
pub fn read_string_column_opt(
    path: &str,
    column_name: &str,
) -> Result<Vec<Option<String>>, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("Failed to read Parquet file '{}': {}", path, e))?;

    // Find column and verify it's a string type
    let schema = builder.schema();
    let (column_index, field) = schema.column_with_name(column_name).ok_or_else(|| {
        let available: Vec<_> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        format!(
            "Column '{}' not found in Parquet file. Available columns: {}",
            column_name,
            available.join(", ")
        )
    })?;

    // Clone the data type so we don't hold a reference to schema/builder
    let data_type = field.data_type().clone();

    // Ensure it's actually a string column
    match &data_type {
        DataType::Utf8 | DataType::LargeUtf8 => {}
        other => {
            return Err(format!(
                "Column '{}' has type {:?}, but must be Utf8 or LargeUtf8 (string)",
                column_name, other
            ));
        }
    }

    let reader = builder
        .build()
        .map_err(|e| format!("Failed to create Parquet reader: {}", e))?;

    let mut strings = Vec::new();

    // Process all record batches
    for batch_result in reader {
        let batch = batch_result.map_err(|e| format!("Failed to read record batch: {}", e))?;

        // Around line 66-76, replace the downcast logic:

        let column = batch.column(column_index);

        // Handle both StringArray and LargeStringArray
        let strings_in_batch: Vec<Option<String>> = match &data_type {
            DataType::Utf8 => {
                let string_array = column
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| "Failed to downcast column to StringArray".to_string())?;

                (0..string_array.len())
                    .map(|i| (!string_array.is_null(i)).then(|| string_array.value(i).to_string()))
                    .collect()
            }
            DataType::LargeUtf8 => {
                use arrow::array::LargeStringArray;
                let string_array = column
                    .as_any()
                    .downcast_ref::<LargeStringArray>()
                    .ok_or_else(|| "Failed to downcast column to LargeStringArray".to_string())?;

                (0..string_array.len())
                    .map(|i| (!string_array.is_null(i)).then(|| string_array.value(i).to_string()))
                    .collect()
            }
            _ => unreachable!("Type already validated"),
        };

        strings.extend(strings_in_batch);
    }

    Ok(strings)
}

/// Write strings to a Parquet file as a single string column
///
/// # Arguments
/// * `path` - Output path for the Parquet file
/// * `column_name` - Name for the string column
/// * `strings` - Vector of strings to write
///
/// # Errors
/// Returns error if file cannot be written or Arrow conversion fails
pub fn write_string_column(
    path: &str,
    column_name: &str,
    strings: Vec<String>,
    metadata: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let strings = strings.into_iter().map(Some).collect();
    write_string_column_with(path, column_name, strings, &[], metadata)
}

/// Write nullable strings as a string column, after the `keep` columns (see [`read_columns`]).
///
/// # Errors
/// Returns error if file cannot be written, a `keep` column's length differs from
/// `strings`, or a `keep` column shares `column_name`.
pub fn write_string_column_with(
    path: &str,
    column_name: &str,
    strings: Vec<Option<String>>,
    keep: &[(FieldRef, ArrayRef)],
    metadata: Option<HashMap<String, String>>,
) -> Result<(), String> {
    // Calculate total byte size to decide between Utf8 and LargeUtf8
    let total_bytes: usize = strings.iter().flatten().map(|s| s.len()).sum();
    let use_large = total_bytes > i32::MAX as usize || strings.len() > i32::MAX as usize;

    let data_type = if use_large {
        DataType::LargeUtf8
    } else {
        DataType::Utf8
    };
    let field = Arc::new(Field::new(column_name, data_type, true));

    // Create appropriate string array based on size
    let array: ArrayRef = if use_large {
        use arrow::array::LargeStringArray;
        Arc::new(LargeStringArray::from(strings))
    } else {
        Arc::new(StringArray::from(strings))
    };

    check_keep_len(keep, array.len())?;
    let schema = output_schema(field, keep, metadata)?;
    let mut writer = open_writer(path, schema.clone())?;
    write_batch(&mut writer, &schema, keep, 0, array)?;
    writer
        .close()
        .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;
    Ok(())
}

/// Write an `extract_lookup` table: `field`, `key` and `value` (the subtree as JSON).
pub fn write_lookup_table(
    path: &str,
    lookup: &[crate::extract::LookupEntry],
) -> Result<(), String> {
    let column = |name: &str, values: Vec<&str>| -> (FieldRef, ArrayRef) {
        (
            Arc::new(Field::new(name, DataType::Utf8, false)),
            Arc::new(StringArray::from(values)),
        )
    };
    let keep = [
        column("field", lookup.iter().map(|e| e.field.as_str()).collect()),
        column("key", lookup.iter().map(|e| e.key.as_str()).collect()),
    ];
    let values = lookup.iter().map(|e| Some(e.value.to_string())).collect();
    write_string_column_with(path, "value", values, &keep, None)
}

/// Read whole columns from a Parquet file, to write alongside a normalised column.
///
/// # Errors
/// Returns error if the file cannot be read or a column doesn't exist.
pub fn read_columns(path: &str, names: &[String]) -> Result<Vec<(FieldRef, ArrayRef)>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let file =
        File::open(path).map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("Failed to read Parquet file '{}': {}", path, e))?;
    let schema = builder.schema().clone();
    let indices = names
        .iter()
        .map(|name| {
            schema
                .index_of(name)
                .map_err(|_| format!("Column '{}' not found in Parquet file", name))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mask = ProjectionMask::roots(builder.parquet_schema(), indices.iter().copied());
    let reader = builder
        .with_projection(mask)
        .build()
        .map_err(|e| format!("Failed to create Parquet reader: {}", e))?;
    let batches = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read record batch: {}", e))?;
    names
        .iter()
        .map(|name| {
            let field = schema.field_with_name(name).map_err(|e| e.to_string())?;
            let chunks: Vec<&dyn Array> = batches
                .iter()
                .map(|b| b.column_by_name(name).unwrap().as_ref())
                .collect();
            let array = if chunks.is_empty() {
                arrow::array::new_empty_array(field.data_type())
            } else {
                arrow::compute::concat(&chunks).map_err(|e| e.to_string())?
            };
            Ok((Arc::new(field.clone()), array))
        })
        .collect()
}

/// Schema of `keep` fields followed by the output `field`.
fn output_schema(
    field: FieldRef,
    keep: &[(FieldRef, ArrayRef)],
    metadata: Option<HashMap<String, String>>,
) -> Result<SchemaRef, String> {
    if let Some((f, _)) = keep.iter().find(|(f, _)| f.name() == field.name()) {
        return Err(format!(
            "Kept column '{}' has the same name as the output column",
            f.name()
        ));
    }
    let fields: Vec<FieldRef> = keep
        .iter()
        .map(|(f, _)| f.clone())
        .chain(std::iter::once(field))
        .collect();
    Ok(Arc::new(Schema::new_with_metadata(
        fields,
        metadata.unwrap_or_default(),
    )))
}

fn check_keep_len(keep: &[(FieldRef, ArrayRef)], rows: usize) -> Result<(), String> {
    match keep.iter().find(|(_, a)| a.len() != rows) {
        Some((f, a)) => Err(format!(
            "Kept column '{}' has {} rows, but the output column has {}",
            f.name(),
            a.len(),
            rows
        )),
        None => Ok(()),
    }
}

fn open_writer(path: &str, schema: SchemaRef) -> Result<ArrowWriter<File>, String> {
    let file = File::create(path)
        .map_err(|e| format!("Failed to create output file '{}': {}", path, e))?;
    let props = WriterProperties::builder().build();
    ArrowWriter::try_new(file, schema, Some(props))
        .map_err(|e| format!("Failed to create Parquet writer: {}", e))
}

/// Write `array` as one batch, with the rows of each `keep` column starting at `offset`.
fn write_batch(
    writer: &mut ArrowWriter<File>,
    schema: &SchemaRef,
    keep: &[(FieldRef, ArrayRef)],
    offset: usize,
    array: ArrayRef,
) -> Result<(), String> {
    let len = array.len();
    let mut columns: Vec<ArrayRef> = keep.iter().map(|(_, a)| a.slice(offset, len)).collect();
    columns.push(array);
    let batch = RecordBatch::try_new(schema.clone(), columns)
        .map_err(|e| format!("Failed to create RecordBatch: {}", e))?;
    writer
        .write(&batch)
        .map_err(|e| format!("Failed to write RecordBatch: {}", e))
}

pub fn read_parquet_metadata(path: &str) -> Result<HashMap<String, String>, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("Failed to read Parquet file '{}': {}", path, e))?;

    let metadata = builder.schema().metadata();
    Ok(metadata
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect())
}

/// Map an Avro type (as inferred in avro mode) to an Arrow type, mirroring
/// `polars_jsonschema_bridge::avro_type_to_polars_type` so the result reads back as the
/// dtype `avro_to_polars_schema` gives: maps become `List<Struct{key, value}>` (the `kv`
/// encoding) and a union takes its first non-null branch.
pub fn avro_to_arrow_type(avro: &serde_json::Value) -> Result<DataType, String> {
    use serde_json::Value;
    match avro {
        Value::String(s) => match s.as_str() {
            "string" => Ok(DataType::Utf8),
            "int" | "long" => Ok(DataType::Int64),
            "float" | "double" => Ok(DataType::Float64),
            "boolean" => Ok(DataType::Boolean),
            "null" => Ok(DataType::Null),
            other => Err(format!("Unsupported Avro type: {}", other)),
        },
        Value::Array(branches) => match branches.iter().find(|t| t.as_str() != Some("null")) {
            Some(branch) => avro_to_arrow_type(branch),
            None => Ok(DataType::Null),
        },
        Value::Object(obj) => match obj.get("type").and_then(Value::as_str) {
            Some("array") => {
                let items = match obj.get("items") {
                    Some(items) => avro_to_arrow_type(items)?,
                    None => DataType::Utf8,
                };
                Ok(DataType::new_list(items, true))
            }
            Some("map") => {
                let values = match obj.get("values") {
                    Some(values) => avro_to_arrow_type(values)?,
                    None => DataType::Utf8,
                };
                let entry = DataType::Struct(
                    vec![
                        Field::new("key", DataType::Utf8, true),
                        Field::new("value", values, true),
                    ]
                    .into(),
                );
                Ok(DataType::new_list(entry, true))
            }
            Some("record") => Ok(DataType::Struct(avro_record_fields(avro)?)),
            _ => Err(format!("Unsupported Avro schema element: {}", avro)),
        },
        _ => Err(format!("Unsupported Avro schema element: {}", avro)),
    }
}

/// The Arrow fields of an Avro record schema, all nullable.
pub fn avro_record_fields(record: &serde_json::Value) -> Result<arrow::datatypes::Fields, String> {
    let mut fields = Vec::new();
    if let Some(avro_fields) = record.get("fields").and_then(|f| f.as_array()) {
        for f in avro_fields {
            if let (Some(name), Some(ftype)) =
                (f.get("name").and_then(|n| n.as_str()), f.get("type"))
            {
                fields.push(Field::new(name, avro_to_arrow_type(ftype)?, true));
            }
        }
    }
    Ok(fields.into())
}

/// Decode rows (JSON objects matching `fields`) into one struct array, with a null
/// struct for each `Value::Null` row.
pub fn values_to_struct_array(
    rows: &[serde_json::Value],
    fields: &arrow::datatypes::Fields,
) -> Result<arrow::array::StructArray, String> {
    use arrow::array::StructArray;
    use arrow::buffer::NullBuffer;

    let schema = Arc::new(Schema::new(fields.clone()));
    let mut decoder = arrow::json::ReaderBuilder::new(schema)
        .with_batch_size(rows.len().max(1))
        .build_decoder()
        .map_err(|e| format!("Failed to build JSON decoder: {}", e))?;
    let empty = serde_json::Value::Object(Default::default());
    let valid: Vec<bool> = rows.iter().map(|r| !r.is_null()).collect();
    let objects: Vec<&serde_json::Value> = rows
        .iter()
        .map(|r| if r.is_null() { &empty } else { r })
        .collect();
    decoder
        .serialize(&objects)
        .map_err(|e| format!("Failed to decode rows to Arrow: {}", e))?;
    let batch = decoder
        .flush()
        .map_err(|e| format!("Failed to decode rows to Arrow: {}", e))?
        .unwrap_or_else(|| RecordBatch::new_empty(Arc::new(Schema::new(fields.clone()))));
    let nulls = valid.contains(&false).then(|| NullBuffer::from(valid));
    StructArray::try_new(fields.clone(), batch.columns().to_vec(), nulls)
        .map_err(|e| format!("Failed to build struct column: {}", e))
}

/// Write struct arrays, in order, as one struct column.
pub fn write_struct_column(
    path: &str,
    column_name: &str,
    arrays: Vec<arrow::array::StructArray>,
    fields: &arrow::datatypes::Fields,
    metadata: Option<HashMap<String, String>>,
) -> Result<(), String> {
    write_struct_column_with(path, column_name, arrays, fields, &[], metadata)
}

/// Write struct arrays, in order, as one struct column after the `keep` columns
/// (see [`read_columns`]), which must have one row per struct row in total.
pub fn write_struct_column_with(
    path: &str,
    column_name: &str,
    arrays: Vec<arrow::array::StructArray>,
    fields: &arrow::datatypes::Fields,
    keep: &[(FieldRef, ArrayRef)],
    metadata: Option<HashMap<String, String>>,
) -> Result<(), String> {
    check_keep_len(keep, arrays.iter().map(|a| a.len()).sum())?;
    let field = Arc::new(Field::new(
        column_name,
        DataType::Struct(fields.clone()),
        true,
    ));
    let schema = output_schema(field, keep, metadata)?;
    let mut writer = open_writer(path, schema.clone())?;
    let mut offset = 0;
    for array in arrays {
        let len = array.len();
        write_batch(&mut writer, &schema, keep, offset, Arc::new(array))?;
        offset += len;
    }
    writer
        .close()
        .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("tests/parquet.rs");
}
