//! Parquet file I/O for reading and writing string columns

use arrow::array::{Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
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
) -> Result<(), String> {
    // Create schema with single nullable string column
    let schema = Schema::new(vec![Field::new(column_name, DataType::Utf8, true)]);
    let schema_ref = Arc::new(schema.clone());

    // Convert Vec<String> to Arrow StringArray
    let string_array = StringArray::from(strings);

    // Create RecordBatch
    let batch = RecordBatch::try_new(schema_ref.clone(), vec![Arc::new(string_array)])
        .map_err(|e| format!("Failed to create RecordBatch: {}", e))?;

    // Open file for writing
    let file = File::create(path)
        .map_err(|e| format!("Failed to create output file '{}': {}", path, e))?;

    // Configure writer properties (can be customized)
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

#[cfg(test)]
mod tests {
    include!("tests/parquet.rs");
}
