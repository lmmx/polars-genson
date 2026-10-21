//! Parquet file I/O for reading and writing string columns

use arrow::array::{Array, ArrayRef, LargeStringArray, StringArray, StringViewArray};
use arrow::datatypes::{DataType, Field, FieldRef, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder, RowSelection,
    RowSelector,
};
use parquet::arrow::{ArrowWriter, ProjectionMask};
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::metadata::{PageIndexPolicy, ParquetMetaData};
use parquet::file::properties::WriterProperties;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Range;
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
    let column = read_string_column_arrays(path, column_name)?;
    Ok(column
        .rows()
        .into_iter()
        .map(|row| row.map(str::to_string))
        .collect())
}

/// A string column read from a Parquet file, as the Arrow arrays it was decoded into: its
/// rows are borrowed from them (see [`StringColumn::rows`]) rather than copied out.
pub struct StringColumn {
    arrays: Vec<ArrayRef>,
}

impl StringColumn {
    /// One entry per row, in order (`None` for null rows).
    pub fn rows(&self) -> Vec<Option<&str>> {
        let mut rows = Vec::with_capacity(self.len());
        for array in &self.arrays {
            if let Some(a) = array.as_any().downcast_ref::<StringArray>() {
                rows.extend(a.iter());
            } else if let Some(a) = array.as_any().downcast_ref::<LargeStringArray>() {
                rows.extend(a.iter());
            } else if let Some(a) = array.as_any().downcast_ref::<StringViewArray>() {
                rows.extend(a.iter());
            }
        }
        rows
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.arrays.iter().map(|a| a.len()).sum()
    }

    /// Whether the column has no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Read a string column from a Parquet file, decoding that column only.
///
/// Where the file has an offset index (Polars and arrow-rs write one, pyarrow does with
/// `write_page_index=True`) or several row groups, the column is decoded by several
/// readers at once, each over its own rows.
///
/// # Errors
/// As for [`read_string_column`].
pub fn read_string_column_arrays(path: &str, column_name: &str) -> Result<StringColumn, String> {
    read_string_column_parts(path, column_name, rayon::current_num_threads() * 4)
}

/// [`read_string_column_arrays`] with the column split into at most `parts` parts.
#[doc(hidden)]
pub fn read_string_column_parts(
    path: &str,
    column_name: &str,
    parts: usize,
) -> Result<StringColumn, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;

    // The offset index says where each of the column's pages starts. A file whose index
    // cannot be read is read as it was before the index was looked for, from its footer
    let load = |policy| {
        let options = ArrowReaderOptions::new().with_offset_index_policy(policy);
        ArrowReaderMetadata::load(&file, options)
    };
    let metadata = load(PageIndexPolicy::Optional)
        .or_else(|_| load(PageIndexPolicy::Skip))
        .map_err(|e| format!("Failed to read Parquet file '{}': {}", path, e))?;

    // Find column and verify it's a string type
    let schema = metadata.schema();
    let (column_index, field) = schema.column_with_name(column_name).ok_or_else(|| {
        let available: Vec<_> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        format!(
            "Column '{}' not found in Parquet file. Available columns: {}",
            column_name,
            available.join(", ")
        )
    })?;
    match field.data_type() {
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {}
        other => {
            return Err(format!(
                "Column '{}' has type {:?}, but must be Utf8 or LargeUtf8 (string)",
                column_name, other
            ));
        }
    }

    // Decode the one column: the others (in wikidata-pq's chunks, every other table's JSON)
    // are not read
    let parquet_schema = metadata.parquet_schema();
    let mask = ProjectionMask::roots(parquet_schema, [column_index]);
    let leaf = (0..parquet_schema.num_columns())
        .find(|&leaf| parquet_schema.get_column_root_idx(leaf) == column_index)
        .expect("a string column has a leaf");
    let ranges = split_rows(metadata.metadata(), leaf, parts);

    // Each part opens the file itself: one `File`'s clones share its position
    let read = |rows: Option<Range<usize>>| -> Result<Vec<ArrayRef>, String> {
        let file = File::open(path)
            .map_err(|e| format!("Failed to open Parquet file '{}': {}", path, e))?;
        let mut builder =
            ParquetRecordBatchReaderBuilder::new_with_metadata(file, metadata.clone())
                .with_projection(mask.clone());
        if let Some(rows) = rows {
            let (groups, selection) = select_rows(metadata.metadata(), rows);
            builder = builder.with_row_groups(groups);
            if let Some(selection) = selection {
                builder = builder.with_row_selection(selection);
            }
        }
        let reader = builder
            .build()
            .map_err(|e| format!("Failed to create Parquet reader: {}", e))?;
        reader
            .map(|batch| {
                batch
                    .map(|b| b.column(0).clone())
                    .map_err(|e| format!("Failed to read record batch: {}", e))
            })
            .collect()
    };
    let parts: Vec<Result<Vec<ArrayRef>, String>> = match ranges {
        Some(ranges) => ranges
            .into_par_iter()
            .map(|rows| read(Some(rows)))
            .collect(),
        None => vec![read(None)],
    };
    // The error reported is the first in row order, as one reader would have met it
    let mut arrays = Vec::new();
    for part in parts {
        arrays.extend(part?);
    }
    Ok(StringColumn { arrays })
}

/// Split the file's rows into at most `parts` ranges of about equal compressed size in the
/// column with leaf index `leaf`, each starting where one of the column's pages starts (by
/// its offset index) or, where the column has no offset index, where a row group starts.
/// `None` when the rows are not split.
fn split_rows(metadata: &ParquetMetaData, leaf: usize, parts: usize) -> Option<Vec<Range<usize>>> {
    // Each place a range can start, with the compressed bytes from there to the next one
    let mut starts: Vec<(usize, u64)> = Vec::new();
    let mut base = 0;
    for (i, group) in metadata.row_groups().iter().enumerate() {
        match metadata.page_index_for_row_group(i).page_locations(leaf) {
            Some(pages) if !pages.is_empty() => starts.extend(pages.iter().map(|page| {
                let first = base + page.first_row_index as usize;
                (first, page.compressed_page_size.max(0) as u64)
            })),
            _ => starts.push((base, group.column(leaf).compressed_size().max(0) as u64)),
        }
        base += group.num_rows() as usize;
    }
    let total_rows = base;
    let total_bytes: u64 = starts.iter().map(|(_, bytes)| bytes).sum();
    if parts < 2 || starts.len() < 2 || total_bytes == 0 {
        return None;
    }
    let mut cuts = vec![0];
    let mut before = 0;
    for &(first, bytes) in &starts {
        // A range ends where the bytes before the next start reach its share of the total
        let next = cuts.len() as u64;
        if before * parts as u64 >= total_bytes * next
            && first > *cuts.last().unwrap()
            && first < total_rows
        {
            cuts.push(first);
        }
        before += bytes;
    }
    if cuts.len() < 2 {
        return None;
    }
    cuts.push(total_rows);
    Some(cuts.windows(2).map(|w| w[0]..w[1]).collect())
}

/// The row groups holding `rows`, and the selection of `rows` within them (`None` when
/// they are the groups' rows exactly).
fn select_rows(
    metadata: &ParquetMetaData,
    rows: Range<usize>,
) -> (Vec<usize>, Option<RowSelection>) {
    let mut groups = Vec::new();
    let (mut first, mut end) = (None, 0);
    let mut base = 0;
    for (i, group) in metadata.row_groups().iter().enumerate() {
        let next = base + group.num_rows() as usize;
        if base < rows.end && next > rows.start {
            groups.push(i);
            first.get_or_insert(base);
            end = next;
        }
        base = next;
    }
    let skip = rows.start - first.unwrap_or(rows.start);
    let tail = end.saturating_sub(rows.end);
    if skip == 0 && tail == 0 {
        return (groups, None);
    }
    let selectors = [
        RowSelector::skip(skip),
        RowSelector::select(rows.len()),
        RowSelector::skip(tail),
    ];
    let selectors = selectors
        .into_iter()
        .filter(|s| s.row_count > 0)
        .collect::<Vec<_>>();
    (groups, Some(RowSelection::from(selectors)))
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
    let batches = vec![batch_of(&schema, keep, 0, array)?];
    let writer = open_writer(path, schema.clone(), false, &batches)?;
    write_batches(writer, &schema, batches)
}

/// Write an `extract_invariants` lookup table: `field`, `key` and `value` (as JSON).
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

/// Write the values removed by `prune`: the `keep` columns at each value's row, its `path`
/// from the row's root (as a JSON array) and its input `value` (as JSON).
pub fn write_pruned_table(
    path: &str,
    rows: &[usize],
    paths: Vec<Option<String>>,
    values: Vec<Option<String>>,
    keep: &[(FieldRef, ArrayRef)],
) -> Result<(), String> {
    let indices = arrow::array::UInt64Array::from_iter_values(rows.iter().map(|&r| r as u64));
    let mut columns = keep
        .iter()
        .map(|(field, array)| {
            arrow::compute::take(array.as_ref(), &indices, None)
                .map(|taken| (field.clone(), taken))
                .map_err(|e| format!("Failed to take kept column '{}': {}", field.name(), e))
        })
        .collect::<Result<Vec<_>, String>>()?;
    columns.push((
        Arc::new(Field::new("path", DataType::Utf8, false)),
        Arc::new(StringArray::from(paths)),
    ));
    write_string_column_with(path, "value", values, &columns, None)
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

/// zstd at level 1: on Wikidata claims, typed output a third the size of uncompressed in
/// the same write time, and normalised JSON an eighth
fn compression() -> Compression {
    Compression::ZSTD(ZstdLevel::try_new(1).expect("1 is a zstd level"))
}

/// A writer for `batches` (which it does not write): with `spec_names`, the Parquet
/// schema's list and map groups take the specification's names (arrow-rs's
/// `coerce_types`, which changes nothing else for genson's types), and leaf columns whose
/// values are mostly distinct are not dictionary encoded (see [`unrepeated_leaves`]).
fn open_writer(
    path: &str,
    schema: SchemaRef,
    spec_names: bool,
    batches: &[RecordBatch],
) -> Result<ArrowWriter<File>, String> {
    let file = File::create(path)
        .map_err(|e| format!("Failed to create output file '{}': {}", path, e))?;
    let mut props = WriterProperties::builder()
        .set_compression(compression())
        .set_coerce_types(spec_names);
    for column in unrepeated_leaves(&schema, spec_names, batches) {
        props = props.set_column_dictionary_enabled(column, false);
    }
    ArrowWriter::try_new(file, schema, Some(props.build()))
        .map_err(|e| format!("Failed to create Parquet writer: {}", e))
}

/// Values sampled from each leaf column, from its first rows
const DICTIONARY_SAMPLE: usize = 16_384;

/// The Parquet leaf columns of `batches` whose sampled values are more than three
/// quarters distinct (of at least 128 sampled), for which a dictionary costs a hash per
/// value and grows the file until arrow-rs gives it up at its page size limit: Polars
/// skips dictionaries at the same ratio, and no other ratio from 0 to 1 measured better
/// across flat, nested and Wikidata tables (docs/journal/2026-10-21-adaptive-dictionary.md).
/// None when the Arrow leaves do not line up with the Parquet leaf columns one to one.
fn unrepeated_leaves(
    schema: &SchemaRef,
    spec_names: bool,
    batches: &[RecordBatch],
) -> Vec<parquet::schema::types::ColumnPath> {
    let Ok(descr) = parquet::arrow::ArrowSchemaConverter::new()
        .with_coerce_types(spec_names)
        .convert(schema)
    else {
        return Vec::new();
    };
    // Each batch's leaf arrays, in Parquet leaf order
    let leaves: Vec<Vec<ArrayRef>> = batches
        .iter()
        .map(|batch| {
            let mut leaves = Vec::new();
            for column in batch.columns() {
                leaf_arrays(column, &mut leaves);
            }
            leaves
        })
        .collect();
    if leaves.iter().any(|l| l.len() != descr.num_columns()) {
        return Vec::new();
    }
    (0..descr.num_columns())
        .into_par_iter()
        .filter(|&i| {
            let mut seen = rustc_hash::FxHashSet::default();
            let mut sampled = 0;
            for batch_leaves in &leaves {
                if !sample(&batch_leaves[i], &mut seen, &mut sampled) {
                    return false;
                }
                if sampled >= DICTIONARY_SAMPLE {
                    break;
                }
            }
            sampled >= 128 && seen.len() * 4 > sampled * 3
        })
        .map(|i| descr.column(i).path().clone())
        .collect()
}

/// The leaf arrays under `array`, depth first, as Parquet's leaf columns are ordered.
fn leaf_arrays(array: &ArrayRef, out: &mut Vec<ArrayRef>) {
    use arrow::array::AsArray;
    match array.data_type() {
        DataType::Struct(_) => array
            .as_struct()
            .columns()
            .iter()
            .for_each(|c| leaf_arrays(c, out)),
        DataType::List(_) => leaf_arrays(array.as_list::<i32>().values(), out),
        DataType::LargeList(_) => leaf_arrays(array.as_list::<i64>().values(), out),
        DataType::FixedSizeList(..) => leaf_arrays(array.as_fixed_size_list().values(), out),
        DataType::Map(..) => {
            let entries = array.as_map().entries();
            entries.columns().iter().for_each(|c| leaf_arrays(c, out))
        }
        _ => out.push(array.clone()),
    }
}

/// Hash `array`'s non-null values into `seen` until `sampled` reaches the sample size;
/// false for a type not sampled (its dictionary setting is left as it is).
fn sample(array: &ArrayRef, seen: &mut rustc_hash::FxHashSet<u64>, sampled: &mut usize) -> bool {
    use arrow::array::AsArray;
    use arrow::datatypes::*;
    use xxhash_rust::xxh64::xxh64;
    let mut take = |bytes: &[u8]| {
        seen.insert(xxh64(bytes, 0));
        *sampled += 1;
        *sampled < DICTIONARY_SAMPLE
    };
    macro_rules! each {
        ($iter:expr, $bytes:expr) => {{
            for v in $iter.flatten() {
                let bytes = $bytes(v);
                if !take(AsRef::<[u8]>::as_ref(&bytes)) {
                    break;
                }
            }
        }};
    }
    match array.data_type() {
        DataType::Utf8 => each!(array.as_string::<i32>().iter(), str::as_bytes),
        DataType::LargeUtf8 => each!(array.as_string::<i64>().iter(), str::as_bytes),
        DataType::Utf8View => each!(array.as_string_view().iter(), str::as_bytes),
        DataType::Binary => each!(array.as_binary::<i32>().iter(), |b| b),
        DataType::LargeBinary => each!(array.as_binary::<i64>().iter(), |b| b),
        DataType::Int32 => each!(array.as_primitive::<Int32Type>().iter(), |v: i32| v
            .to_le_bytes()),
        DataType::Int64 => each!(array.as_primitive::<Int64Type>().iter(), |v: i64| v
            .to_le_bytes()),
        DataType::UInt32 => each!(array.as_primitive::<UInt32Type>().iter(), |v: u32| v
            .to_le_bytes()),
        DataType::UInt64 => each!(array.as_primitive::<UInt64Type>().iter(), |v: u64| v
            .to_le_bytes()),
        DataType::Float32 => each!(array.as_primitive::<Float32Type>().iter(), |v: f32| v
            .to_le_bytes()),
        DataType::Float64 => each!(array.as_primitive::<Float64Type>().iter(), |v: f64| v
            .to_le_bytes()),
        _ => return false,
    }
    true
}

/// `array` as one batch, with the rows of each `keep` column starting at `offset`.
fn batch_of(
    schema: &SchemaRef,
    keep: &[(FieldRef, ArrayRef)],
    offset: usize,
    array: ArrayRef,
) -> Result<RecordBatch, String> {
    let len = array.len();
    let mut columns: Vec<ArrayRef> = keep.iter().map(|(_, a)| a.slice(offset, len)).collect();
    columns.push(array);
    RecordBatch::try_new(schema.clone(), columns)
        .map_err(|e| format!("Failed to create RecordBatch: {}", e))
}

/// Write `batches` with `writer` and close it: the same file, byte for byte, as
/// `ArrowWriter::write` of each batch and `close` write, with the work done in parallel.
/// The rows go to row groups as there, `max_row_group_row_count` to each (`open_writer`
/// sets no byte limit or content-defined chunking), and each column's writer is handed its
/// leaf of each batch in turn, as `ArrowWriter`'s are.
///
/// Row groups are encoded a few at a time, up to one per thread, each row group's leaf
/// columns (and each batch's levels) a task of their own: a wide table's columns fill the
/// threads within one row group, and a narrow table's row groups fill them side by side.
/// The encoded chunks are then written in row group order, so at most that many row
/// groups' levels and encoded chunks are held at once.
fn write_batches(
    writer: ArrowWriter<File>,
    schema: &SchemaRef,
    batches: Vec<RecordBatch>,
) -> Result<(), String> {
    use parquet::arrow::arrow_writer::{
        compute_leaves, ArrowColumnChunk, ArrowColumnWriter, ArrowLeafColumn,
    };
    let failed = |e: parquet::errors::ParquetError| format!("Failed to write Parquet: {}", e);
    let (mut file_writer, factory) = writer.into_serialized_writer().map_err(failed)?;
    let props = file_writer.properties();
    debug_assert!(props.max_row_group_bytes().is_none());
    debug_assert!(props.content_defined_chunking().is_none());
    let max_rows = props.max_row_group_row_count().unwrap_or(usize::MAX);
    let mut row_groups: Vec<Vec<RecordBatch>> = Vec::new();
    let mut current: Vec<RecordBatch> = Vec::new();
    let mut buffered = 0;
    for batch in batches {
        let mut remaining = batch;
        while remaining.num_rows() > 0 {
            let rows = (max_rows - buffered).min(remaining.num_rows());
            current.push(remaining.slice(0, rows));
            remaining = remaining.slice(rows, remaining.num_rows() - rows);
            buffered += rows;
            if buffered >= max_rows {
                row_groups.push(std::mem::take(&mut current));
                buffered = 0;
            }
        }
    }
    if !current.is_empty() {
        row_groups.push(current);
    }
    let in_flight = rayon::current_num_threads().max(1);
    for (wave, groups) in row_groups.chunks(in_flight).enumerate() {
        let first = wave * in_flight;
        let writers: Vec<Vec<ArrowColumnWriter>> = (0..groups.len())
            .map(|g| factory.create_column_writers(first + g))
            .collect::<Result<_, _>>()
            .map_err(failed)?;
        let columns = writers.first().map_or(0, Vec::len);
        // Each row group's batches' leaf columns, in the order of the column writers
        let leaves: Vec<Vec<Vec<ArrowLeafColumn>>> = groups
            .par_iter()
            .map(|batches| {
                batches
                    .par_iter()
                    .map(|batch| {
                        let mut leaves = Vec::with_capacity(columns);
                        for (field, column) in schema.fields().iter().zip(batch.columns()) {
                            leaves.extend(compute_leaves(field, column)?);
                        }
                        Ok(leaves)
                    })
                    .collect::<Result<_, parquet::errors::ParquetError>>()
            })
            .collect::<Result<_, _>>()
            .map_err(failed)?;
        // One task per (row group, leaf column), collected in row group then column order
        let tasks: Vec<(usize, usize, ArrowColumnWriter)> = writers
            .into_iter()
            .enumerate()
            .flat_map(|(g, ws)| ws.into_iter().enumerate().map(move |(i, w)| (g, i, w)))
            .collect();
        let chunks: Vec<ArrowColumnChunk> = tasks
            .into_par_iter()
            .map(|(g, i, mut writer)| {
                for batch_leaves in &leaves[g] {
                    writer.write(&batch_leaves[i])?;
                }
                writer.close()
            })
            .collect::<Result<_, _>>()
            .map_err(failed)?;
        let mut chunks = chunks.into_iter();
        for _ in groups {
            let mut row_group = file_writer.next_row_group().map_err(failed)?;
            for chunk in chunks.by_ref().take(columns) {
                chunk.append_to_row_group(&mut row_group).map_err(failed)?;
            }
            row_group.close().map_err(failed)?;
        }
    }
    file_writer
        .close()
        .map_err(|e| format!("Failed to close Parquet writer: {}", e))?;
    Ok(())
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
/// dtype `avro_to_polars_schema` gives: maps become an Arrow `Map` when `native_map` is set
/// (the `mapping` encoding, a `pl.Map` in Polars 2) or else `List<Struct{key, value}>`
/// (the `kv` encoding), and a union takes its first non-null branch.
pub fn avro_to_arrow_type(avro: &serde_json::Value, native_map: bool) -> Result<DataType, String> {
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
            Some(branch) => avro_to_arrow_type(branch, native_map),
            None => Ok(DataType::Null),
        },
        Value::Object(obj) => match obj.get("type").and_then(Value::as_str) {
            Some("array") => {
                let items = match obj.get("items") {
                    Some(items) => avro_to_arrow_type(items, native_map)?,
                    None => DataType::Utf8,
                };
                Ok(DataType::new_list(items, true))
            }
            Some("map") => {
                let values = match obj.get("values") {
                    Some(values) => avro_to_arrow_type(values, native_map)?,
                    None => DataType::Utf8,
                };
                if native_map {
                    // Map keys are never null
                    let entries = DataType::Struct(
                        vec![
                            Field::new("key", DataType::Utf8, false),
                            Field::new("value", values, true),
                        ]
                        .into(),
                    );
                    return Ok(DataType::Map(
                        Arc::new(Field::new("entries", entries, false)),
                        false,
                    ));
                }
                let entry = DataType::Struct(
                    vec![
                        Field::new("key", DataType::Utf8, true),
                        Field::new("value", values, true),
                    ]
                    .into(),
                );
                Ok(DataType::new_list(entry, true))
            }
            Some("record") => Ok(DataType::Struct(avro_record_fields(avro, native_map)?)),
            _ => Err(format!("Unsupported Avro schema element: {}", avro)),
        },
        _ => Err(format!("Unsupported Avro schema element: {}", avro)),
    }
}

/// The Arrow fields of an Avro record schema, all nullable.
pub fn avro_record_fields(
    record: &serde_json::Value,
    native_map: bool,
) -> Result<arrow::datatypes::Fields, String> {
    let mut fields = Vec::new();
    if let Some(avro_fields) = record.get("fields").and_then(|f| f.as_array()) {
        for f in avro_fields {
            if let (Some(name), Some(ftype)) =
                (f.get("name").and_then(|n| n.as_str()), f.get("type"))
            {
                fields.push(Field::new(
                    name,
                    avro_to_arrow_type(ftype, native_map)?,
                    true,
                ));
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
    write_struct_columns(
        path,
        column_name,
        arrays,
        fields,
        keep,
        metadata,
        StructLayout::default(),
    )
}

/// How [`write_struct_columns`] lays out its output.
#[derive(Clone, Copy, Debug, Default)]
pub struct StructLayout {
    /// The struct's fields as columns of their own after the `keep` columns (a null row's
    /// fields as nulls, as Polars' `unnest` makes them), in place of the one struct column
    pub unnest: bool,
    /// The Parquet specification's names for the groups of lists (`list`, `element`) and
    /// maps (`key_value`), which Polars and pyarrow write, in place of the Arrow fields'
    pub spec_names: bool,
}

/// As [`write_struct_column_with`], laid out as `layout` says.
pub fn write_struct_columns(
    path: &str,
    column_name: &str,
    arrays: Vec<arrow::array::StructArray>,
    fields: &arrow::datatypes::Fields,
    keep: &[(FieldRef, ArrayRef)],
    metadata: Option<HashMap<String, String>>,
    layout: StructLayout,
) -> Result<(), String> {
    check_keep_len(keep, arrays.iter().map(|a| a.len()).sum())?;
    let field = Arc::new(Field::new(
        column_name,
        DataType::Struct(fields.clone()),
        true,
    ));
    let schema = if layout.unnest {
        let columns: Vec<FieldRef> = fields
            .iter()
            .map(|f| Arc::new(f.as_ref().clone().with_nullable(true)))
            .collect();
        unnested_schema(&columns, keep, metadata)?
    } else {
        output_schema(field, keep, metadata)?
    };
    let mut offset = 0;
    let mut batches = Vec::with_capacity(arrays.len());
    for array in arrays {
        let len = array.len();
        let batch = if layout.unnest {
            let mut columns: Vec<ArrayRef> =
                keep.iter().map(|(_, a)| a.slice(offset, len)).collect();
            columns.extend(crate::conform::unnest(&array)?.into_iter().map(|(_, c)| c));
            RecordBatch::try_new(schema.clone(), columns)
                .map_err(|e| format!("Failed to create RecordBatch: {}", e))?
        } else {
            batch_of(&schema, keep, offset, Arc::new(array))?
        };
        batches.push(batch);
        offset += len;
    }
    let writer = open_writer(path, schema.clone(), layout.spec_names, &batches)?;
    write_batches(writer, &schema, batches)
}

/// The `keep` columns' fields, then `columns`, which must not repeat their names.
fn unnested_schema(
    columns: &[FieldRef],
    keep: &[(FieldRef, ArrayRef)],
    metadata: Option<HashMap<String, String>>,
) -> Result<SchemaRef, String> {
    if let Some(f) = columns
        .iter()
        .find(|f| keep.iter().any(|(k, _)| k.name() == f.name()))
    {
        return Err(format!(
            "Kept column '{}' has the same name as a field of the unnested output",
            f.name()
        ));
    }
    let fields: Vec<FieldRef> = keep
        .iter()
        .map(|(f, _)| f.clone())
        .chain(columns.iter().cloned())
        .collect();
    Ok(Arc::new(Schema::new_with_metadata(
        fields,
        metadata.unwrap_or_default(),
    )))
}

#[cfg(test)]
mod tests {
    include!("tests/parquet.rs");
}
