//! Parquet file I/O for reading and writing string columns

use arrow::array::{Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
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
        let strings_in_batch: Vec<String> = match &data_type {
            DataType::Utf8 => {
                let string_array = column
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| "Failed to downcast column to StringArray".to_string())?;

                (0..string_array.len())
                    .filter_map(|i| {
                        if !string_array.is_null(i) {
                            Some(string_array.value(i).to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            DataType::LargeUtf8 => {
                use arrow::array::LargeStringArray;
                let string_array = column
                    .as_any()
                    .downcast_ref::<LargeStringArray>()
                    .ok_or_else(|| "Failed to downcast column to LargeStringArray".to_string())?;

                (0..string_array.len())
                    .filter_map(|i| {
                        if !string_array.is_null(i) {
                            Some(string_array.value(i).to_string())
                        } else {
                            None
                        }
                    })
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
    // Calculate total byte size to decide between Utf8 and LargeUtf8
    let total_bytes: usize = strings.iter().map(|s| s.len()).sum();
    let use_large = total_bytes > i32::MAX as usize || strings.len() > i32::MAX as usize;

    // Create schema with appropriate string type
    let data_type = if use_large {
        DataType::LargeUtf8
    } else {
        DataType::Utf8
    };

    let field = Field::new(column_name, data_type.clone(), true);

    // Create schema with metadata if provided
    let schema = if let Some(meta) = metadata {
        Schema::new_with_metadata(vec![field], meta)
    } else {
        Schema::new(vec![field])
    };

    let schema_ref = Arc::new(schema);

    // Create appropriate string array based on size
    let array: Arc<dyn Array> = if use_large {
        use arrow::array::LargeStringArray;
        Arc::new(LargeStringArray::from(strings))
    } else {
        Arc::new(StringArray::from(strings))
    };

    // Create RecordBatch
    let batch = RecordBatch::try_new(schema_ref.clone(), vec![array])
        .map_err(|e| format!("Failed to create RecordBatch: {}", e))?;

    // Open file for writing
    let file = File::create(path)
        .map_err(|e| format!("Failed to create output file '{}': {}", path, e))?;

    // Configure writer properties
    let props = WriterProperties::builder().build();

    // Create Arrow writer
    let mut writer = ArrowWriter::try_new(file, schema_ref, Some(props))
        .map_err(|e| format!("Failed to create Parquet writer: {}", e))?;

    // Write the batch
    writer
        .write(&batch)
        .map_err(|e| format!("Failed to write RecordBatch: {}", e))?;

    // Close and finalize
    writer
        .close()
        .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;

    Ok(())
}

pub fn read_parquet_metadata(path: &str) -> Result<HashMap<String, String>, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("Failed to read Parquet file '{}': {}", path, e))?;

    let metadata = builder.schema().metadata().clone();
    Ok(metadata)
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
    let field = Field::new(column_name, DataType::Struct(fields.clone()), true);
    let schema = Arc::new(match metadata {
        Some(meta) => Schema::new_with_metadata(vec![field], meta),
        None => Schema::new(vec![field]),
    });
    let file = File::create(path)
        .map_err(|e| format!("Failed to create output file '{}': {}", path, e))?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))
        .map_err(|e| format!("Failed to create Parquet writer: {}", e))?;
    for array in arrays {
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)])
            .map_err(|e| format!("Failed to create RecordBatch: {}", e))?;
        writer
            .write(&batch)
            .map_err(|e| format!("Failed to write RecordBatch: {}", e))?;
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
