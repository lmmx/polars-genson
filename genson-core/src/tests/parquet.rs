// genson-core/src/tests/parquet.rs
use super::*;
use tempfile::NamedTempFile;

#[test]
fn test_write_and_read_roundtrip() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![
        r#"{"name": "Alice", "age": 30}"#.to_string(),
        r#"{"name": "Bob", "age": 25}"#.to_string(),
    ];

    // Write
    write_string_column(path, "json_data", test_strings.clone(), None).unwrap();

    // Read back
    let result = read_string_column(path, "json_data").unwrap();

    assert_eq!(result, test_strings);
}

#[test]
fn test_read_nonexistent_column() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec!["test".to_string()];
    write_string_column(path, "data", test_strings, None).unwrap();

    let result = read_string_column(path, "wrong_name");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_write_and_read_with_metadata() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![
        r#"{"name": "Alice"}"#.to_string(),
        r#"{"name": "Bob"}"#.to_string(),
    ];

    let mut metadata = HashMap::new();
    metadata.insert("test_key".to_string(), "test_value".to_string());
    metadata.insert("schema_version".to_string(), "1.0".to_string());

    // Write with metadata
    write_string_column(path, "data", test_strings.clone(), Some(metadata.clone())).unwrap();

    // Read data back
    let result_strings = read_string_column(path, "data").unwrap();
    assert_eq!(result_strings, test_strings);

    // Read metadata back
    let result_metadata = read_parquet_metadata(path).unwrap();
    assert_eq!(result_metadata.get("test_key"), Some(&"test_value".to_string()));
    assert_eq!(result_metadata.get("schema_version"), Some(&"1.0".to_string()));
}

#[test]
fn test_write_without_metadata_read_empty_metadata() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec!["test".to_string()];
    write_string_column(path, "data", test_strings, None).unwrap();

    let metadata = read_parquet_metadata(path).unwrap();
    assert!(metadata.is_empty());
}

#[test]
#[ignore]
fn test_large_string_array_automatic_selection() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // Create strings that exceed i32::MAX in total bytes
    let large_string = "a".repeat(1_000_000); // 1MB string
    let count = 3000; // 3GB total
    let test_strings: Vec<String> = (0..count).map(|_| large_string.clone()).collect();

    // Should automatically use LargeUtf8
    write_string_column(path, "large_data", test_strings.clone(), None).unwrap();

    // Read back and verify first few (reading all 3GB would be slow)
    let result = read_string_column(path, "large_data").unwrap();
    assert_eq!(result.len(), count);
    assert_eq!(result[0], test_strings[0]);
}

#[test]
fn test_metadata_with_complex_json_schema() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let test_strings = vec![r#"{"a":1}"#.to_string()];

    // Simulate storing a complex schema as metadata
    let complex_schema = r#"{"type":"object","properties":{"a":{"type":"integer"}}}"#;
    let mut metadata = HashMap::new();
    metadata.insert("genson_avro_schema".to_string(), complex_schema.to_string());
    metadata.insert("genson_normalise_config".to_string(), r#"{"empty_as_null":true,"coerce_string":false,"map_encoding":"kv","wrap_root":null}"#.to_string());

    write_string_column(path, "data", test_strings, Some(metadata.clone())).unwrap();

    let result_metadata = read_parquet_metadata(path).unwrap();
    assert_eq!(result_metadata.get("genson_avro_schema"), Some(&complex_schema.to_string()));

    // Verify we can parse it back
    let schema_json = result_metadata.get("genson_avro_schema").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(schema_json).unwrap();
    assert_eq!(parsed["type"], "object");
}

#[test]
fn test_null_rows_and_kept_columns_roundtrip() {
    let src = NamedTempFile::new().unwrap();
    let src_path = src.path().to_str().unwrap();
    let ids: ArrayRef = Arc::new(StringArray::from(vec!["Q1", "Q2", "Q3"]));
    let id_field: FieldRef = Arc::new(Field::new("id", DataType::Utf8, true));
    let strings = vec![Some(r#"{"a": 1}"#.to_string()), None, Some("{}".to_string())];
    let keep = vec![(id_field, ids.clone())];
    write_string_column_with(src_path, "json", strings.clone(), &keep, None).unwrap();

    // Nulls are kept by the _opt reader and skipped by the plain one
    assert_eq!(read_string_column_opt(src_path, "json").unwrap(), strings);
    assert_eq!(read_string_column(src_path, "json").unwrap().len(), 2);

    // The arrays reader decodes the one column and borrows its rows, nulls included
    let column = read_string_column_arrays(src_path, "json").unwrap();
    let borrowed: Vec<Option<String>> = column
        .rows()
        .into_iter()
        .map(|row| row.map(String::from))
        .collect();
    assert_eq!(borrowed, strings);
    assert_eq!(column.len(), 3);
    let ids_read = read_string_column_arrays(src_path, "id").unwrap();
    assert_eq!(ids_read.rows(), vec![Some("Q1"), Some("Q2"), Some("Q3")]);

    let kept = read_columns(src_path, &["id".to_string()]).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].0.name(), "id");
    assert_eq!(kept[0].1.as_ref(), ids.as_ref());

    // A kept column with the wrong row count is rejected
    let out = NamedTempFile::new().unwrap();
    let short = vec![(kept[0].0.clone(), kept[0].1.slice(0, 2))];
    let err = write_string_column_with(out.path().to_str().unwrap(), "json", strings, &short, None);
    assert!(err.unwrap_err().contains("has 2 rows"));
}

#[test]
fn test_output_is_zstd_compressed() {
    use parquet::file::reader::{FileReader, SerializedFileReader};
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();
    let strings: Vec<String> = (0..100).map(|i| format!(r#"{{"a": {i}}}"#)).collect();
    write_string_column(path, "json", strings.clone(), None).unwrap();

    let reader = SerializedFileReader::new(File::open(path).unwrap()).unwrap();
    let column = reader.metadata().row_group(0).column(0);
    assert_eq!(column.compression(), compression());
    assert_eq!(read_string_column(path, "json").unwrap(), strings);
}

/// `rows` rows of an `id` column and a struct column, `claims`, of a string, a list of
/// integers and a struct of a float, any of them null, in batches of uneven sizes (some
/// of them empty).
fn struct_batches(rows: usize, seed: u64) -> (SchemaRef, Vec<RecordBatch>) {
    use arrow::array::{Float64Array, ListArray, StructArray};
    use arrow::buffer::NullBuffer;
    use arrow::datatypes::{Fields, Int64Type};
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    let draws: Vec<[usize; 4]> = (0..rows).map(|_| [next(), next(), next(), next()]).collect();
    let s: ArrayRef = Arc::new(StringArray::from_iter(
        draws.iter().map(|d| (d[0] % 5 != 0).then(|| "s".repeat(d[0] % 13))),
    ));
    let xs: ArrayRef = Arc::new(ListArray::from_iter_primitive::<Int64Type, _, _>(draws.iter().map(|d| {
        (d[1] % 7 != 0).then(|| (0..d[1] % 6).map(|j| (j != 3).then_some((d[1] * j) as i64)).collect::<Vec<_>>())
    })));
    let v: ArrayRef = Arc::new(Float64Array::from_iter(
        draws.iter().map(|d| (d[2] % 4 != 0).then_some(d[2] as f64 / 7.0)),
    ));
    let inner_fields = Fields::from(vec![Field::new("v", DataType::Float64, true)]);
    let inner_nulls = NullBuffer::from(draws.iter().map(|d| d[3] % 6 != 0).collect::<Vec<_>>());
    let inner: ArrayRef = Arc::new(StructArray::try_new(inner_fields.clone(), vec![v], Some(inner_nulls)).unwrap());
    let claims_fields = Fields::from(vec![
        Field::new("s", DataType::Utf8, true),
        Field::new("xs", xs.data_type().clone(), true),
        Field::new("inner", DataType::Struct(inner_fields), true),
    ]);
    let claims_nulls = NullBuffer::from(draws.iter().map(|d| d[3] % 9 != 0).collect::<Vec<_>>());
    let claims: ArrayRef =
        Arc::new(StructArray::try_new(claims_fields.clone(), vec![s, xs, inner], Some(claims_nulls)).unwrap());
    let ids: ArrayRef = Arc::new(StringArray::from_iter_values((0..rows).map(|i| format!("Q{i}"))));
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("claims", DataType::Struct(claims_fields), true),
    ]));
    let mut batches = vec![RecordBatch::new_empty(schema.clone())];
    let mut from = 0;
    while from < rows {
        let len = (next() % 23).min(rows - from);
        batches.push(RecordBatch::try_new(schema.clone(), vec![ids.slice(from, len), claims.slice(from, len)]).unwrap());
        from += len;
    }
    (schema, batches)
}

#[test]
fn test_write_batches_matches_arrow_writer() {
    use parquet::file::properties::EnabledStatistics;
    let zstd = || WriterProperties::builder().set_compression(compression());
    let layouts = [
        ("as opened", zstd().build()),
        (
            "small pages, row groups of 37",
            zstd()
                .set_write_batch_size(3)
                .set_data_page_row_count_limit(4)
                .set_max_row_group_row_count(Some(37))
                .build(),
        ),
        (
            "row groups of 1, page statistics, bloom filters",
            WriterProperties::builder()
                .set_max_row_group_row_count(Some(1))
                .set_statistics_enabled(EnabledStatistics::Page)
                .set_bloom_filter_enabled(true)
                .build(),
        ),
        (
            "no dictionary, no offset index",
            zstd()
                .set_dictionary_enabled(false)
                .set_statistics_enabled(EnabledStatistics::Chunk)
                .set_offset_index_disabled(true)
                .set_max_row_group_row_count(Some(50))
                .build(),
        ),
    ];
    // Row groups are encoded up to one per thread at a time: waves of one row group, uneven
    // waves, and waves wider than the row groups there are
    let pools = [1, 3, 8].map(|n| rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap());
    for (layout, props) in layouts {
        for rows in [0, 1, 2, 37, 38, 74, 300] {
            let (schema, batches) = struct_batches(rows, rows as u64 + 1);
            let one = NamedTempFile::new().unwrap();
            let mut writer =
                ArrowWriter::try_new(File::create(one.path()).unwrap(), schema.clone(), Some(props.clone())).unwrap();
            for batch in &batches {
                writer.write(batch).unwrap();
            }
            writer.close().unwrap();
            let one = std::fs::read(one.path()).unwrap();
            for pool in &pools {
                let parallel = NamedTempFile::new().unwrap();
                let writer = ArrowWriter::try_new(File::create(parallel.path()).unwrap(), schema.clone(), Some(props.clone()))
                    .unwrap();
                pool.install(|| write_batches(writer, &schema, batches.clone())).unwrap();
                let parallel = std::fs::read(parallel.path()).unwrap();
                let threads = pool.current_num_threads();
                assert!(one == parallel, "{layout}, {rows} rows, {threads} threads: {} and {} bytes", one.len(), parallel.len());
            }
        }
    }
}

/// A file of `rows` rows with an `id` column and a `json` column of uneven, sometimes
/// null strings, in the given layout.
fn split_file(rows: usize, props: WriterProperties, large: bool) -> (NamedTempFile, Vec<Option<String>>) {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let strings: Vec<Option<String>> = (0..rows)
        .map(|i| match next() % 10 {
            0 => None,
            n => Some(format!(r#"{{"row": {i}, "pad": "{}"}}"#, "x".repeat((n * n * n) as usize * 7))),
        })
        .collect();
    let ids: ArrayRef = Arc::new(StringArray::from_iter_values((0..rows).map(|i| format!("Q{i}"))));
    let json: ArrayRef = if large {
        Arc::new(LargeStringArray::from(strings.clone()))
    } else {
        Arc::new(StringArray::from(strings.clone()))
    };
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("json", json.data_type().clone(), true),
    ]));
    let file = NamedTempFile::new().unwrap();
    let mut writer = ArrowWriter::try_new(File::create(file.path()).unwrap(), schema.clone(), Some(props)).unwrap();
    // Written in small batches, so that pages and row groups end where the limits say
    for from in (0..rows).step_by(5) {
        let len = 5.min(rows - from);
        let batch = RecordBatch::try_new(schema.clone(), vec![ids.slice(from, len), json.slice(from, len)]).unwrap();
        writer.write(&batch).unwrap();
    }
    writer.close().unwrap();
    (file, strings)
}

#[test]
fn test_split_read_matches_one_reader() {
    use parquet::file::properties::EnabledStatistics;
    use parquet::file::reader::{FileReader, SerializedFileReader};
    let small_pages = || {
        WriterProperties::builder()
            .set_write_batch_size(3)
            .set_data_page_row_count_limit(4)
            .set_data_page_size_limit(2000)
    };
    let no_offset_index = |b: parquet::file::properties::WriterPropertiesBuilder| {
        b.set_statistics_enabled(EnabledStatistics::Chunk).set_offset_index_disabled(true)
    };
    // (layout, whether the rows are split)
    let layouts = [
        ("pages, one row group", small_pages().build(), true),
        ("pages, several row groups", small_pages().set_max_row_group_row_count(Some(37)).build(), true),
        ("row groups, no offset index", no_offset_index(small_pages().set_max_row_group_row_count(Some(37))).build(), true),
        ("one row group, no offset index", no_offset_index(small_pages()).build(), false),
    ];
    for (layout, props, splits) in layouts {
        for large in [false, true] {
            for rows in [0, 1, 2, 9, 200] {
                let (file, strings) = split_file(rows, props.clone(), large);
                let path = file.path().to_str().unwrap();

                let reader = SerializedFileReader::new(File::open(path).unwrap()).unwrap();
                let options = ArrowReaderOptions::new().with_offset_index_policy(PageIndexPolicy::Optional);
                let metadata = ArrowReaderMetadata::load(&File::open(path).unwrap(), options).unwrap();
                let ranges = split_rows(metadata.metadata(), 1, 16);
                if rows == 200 {
                    assert_eq!(ranges.is_some(), splits, "{layout}: {}", reader.metadata().num_row_groups());
                }
                // The ranges are the rows in order, each one not empty
                if let Some(ranges) = &ranges {
                    assert_eq!(ranges.first().unwrap().start, 0);
                    assert_eq!(ranges.last().unwrap().end, rows);
                    assert!(ranges.windows(2).all(|w| w[0].end == w[1].start));
                    assert!(ranges.iter().all(|r| !r.is_empty()));
                    assert!(ranges.len() <= 16);
                }

                for parts in [1, 2, 3, 5, 16, 64, 1000] {
                    let column = read_string_column_parts(path, "json", parts).unwrap();
                    let read: Vec<Option<String>> =
                        column.rows().into_iter().map(|r| r.map(String::from)).collect();
                    assert_eq!(read, strings, "{layout}, {rows} rows, {parts} parts, large {large}");
                    let ids = read_string_column_parts(path, "id", parts).unwrap();
                    assert_eq!(ids.len(), rows);
                }
            }
        }
    }
}

#[test]
fn test_split_read_errors_as_one_reader() {
    let (file, _) = split_file(50, WriterProperties::builder().build(), false);
    let path = file.path().to_str().unwrap();
    for parts in [1, 8] {
        let err = read_string_column_parts(path, "nope", parts).err().unwrap();
        assert!(err.starts_with("Column 'nope' not found"), "{err}");
    }
    let err = read_string_column_arrays("/nonexistent.parquet", "json").err().unwrap();
    assert!(err.starts_with("Failed to open Parquet file"), "{err}");
}

/// Whether each leaf column chunk of the file's first row group has a dictionary page,
/// by its dotted Parquet path.
fn dictionary_pages(path: &std::path::Path) -> HashMap<String, bool> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap()).unwrap();
    let row_group = reader.metadata().row_group(0);
    row_group
        .columns()
        .iter()
        .map(|c| (c.column_path().string(), c.dictionary_page_offset().is_some()))
        .collect()
}

/// A dictionary is kept for a leaf whose sampled values repeat and left out for one whose
/// values are mostly distinct, at any depth: a top-level string, an integer and a string
/// in a list of structs, and a JSON string column.
#[test]
fn test_mostly_distinct_leaves_are_not_dictionary_encoded() {
    use arrow::array::{Int64Array, ListArray, StructArray};
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::Fields;
    let rows = 2_000;
    let unique: ArrayRef = Arc::new(StringArray::from_iter_values((0..rows).map(|i| format!("Q{i}"))));
    let repeated: ArrayRef = Arc::new(StringArray::from_iter_values((0..rows).map(|i| format!("P{}", i % 7))));
    // A list of two structs per row: `n` unique, `kind` one of three
    let n: ArrayRef = Arc::new(Int64Array::from_iter_values((0..2 * rows as i64).map(|i| i * 7919)));
    let kind: ArrayRef = Arc::new(StringArray::from_iter_values((0..2 * rows).map(|i| ["a", "b", "c"][i % 3])));
    let item_fields = Fields::from(vec![Field::new("n", DataType::Int64, true), Field::new("kind", DataType::Utf8, true)]);
    let items = StructArray::new(item_fields.clone(), vec![n, kind], None);
    let list: ArrayRef = Arc::new(ListArray::new(
        Arc::new(Field::new("item", DataType::Struct(item_fields), true)),
        OffsetBuffer::from_lengths(std::iter::repeat_n(2, rows)),
        Arc::new(items),
        None,
    ));
    let fields = Fields::from(vec![
        Field::new("unique", DataType::Utf8, true),
        Field::new("repeated", DataType::Utf8, true),
        Field::new("items", list.data_type().clone(), true),
    ]);
    let array = StructArray::new(fields.clone(), vec![unique, repeated, list], None);
    let file = NamedTempFile::new().unwrap();
    let layout = StructLayout { unnest: true, spec_names: true };
    write_struct_columns(file.path().to_str().unwrap(), "x", vec![array], &fields, &[], None, layout).unwrap();
    let pages = dictionary_pages(file.path());
    assert!(!pages["unique"], "{pages:?}");
    assert!(pages["repeated"], "{pages:?}");
    assert!(!pages["items.list.element.n"], "{pages:?}");
    assert!(pages["items.list.element.kind"], "{pages:?}");

    let json = NamedTempFile::new().unwrap();
    let strings = (0..rows).map(|i| Some(format!("{{\"id\": {i}}}"))).collect();
    write_string_column_with(json.path().to_str().unwrap(), "json", strings, &[], None).unwrap();
    assert!(!dictionary_pages(json.path())["json"]);
}

/// A flat column's data pages end at the page size limit (1 MiB), not every 20,000 rows as
/// arrow-rs's default would have them: 300,000 distinct Int64 values (2.4 MB, written
/// plain) take three pages, and read back unchanged.
#[test]
fn test_flat_pages_end_at_the_page_size_limit() {
    use arrow::array::{Int64Array, StructArray};
    use arrow::datatypes::Fields;
    let rows = 300_000;
    let values = Int64Array::from_iter_values((0..rows as i64).map(|i| i * 7919));
    let fields = Fields::from(vec![Field::new("n", DataType::Int64, true)]);
    let array = StructArray::new(fields.clone(), vec![Arc::new(values.clone())], None);
    let file = NamedTempFile::new().unwrap();
    let layout = StructLayout { unnest: true, spec_names: true };
    write_struct_columns(file.path().to_str().unwrap(), "x", vec![array], &fields, &[], None, layout).unwrap();

    let options = ArrowReaderOptions::new().with_offset_index_policy(PageIndexPolicy::Required);
    let metadata = ArrowReaderMetadata::load(&File::open(file.path()).unwrap(), options).unwrap();
    let pages = metadata.metadata().page_index_for_row_group(0).page_locations(0).unwrap().clone();
    let starts: Vec<usize> = pages.iter().map(|p| p.first_row_index as usize).chain([rows]).collect();
    let page_rows: Vec<usize> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    assert_eq!(page_rows.len(), 3, "{page_rows:?}");
    assert!(page_rows[..page_rows.len() - 1].iter().all(|&n| n > 20_000), "{page_rows:?}");

    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(file.path()).unwrap()).unwrap().build().unwrap();
    let read: Vec<i64> = reader
        .flat_map(|batch| {
            let batch = batch.unwrap();
            batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap().values().to_vec()
        })
        .collect();
    assert_eq!(read, values.values().to_vec());
}

#[test]
fn test_write_arrow_batches_round_trip() {
    use arrow::array::{Int64Array, ListArray, StringArray};
    use arrow::datatypes::{DataType, Field, Int64Type, Schema};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::sync::Arc;

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, true),
        Field::new(
            "values",
            DataType::List(Arc::new(Field::new_list_field(DataType::Int64, true))),
            true,
        ),
        Field::new("n", DataType::Int64, false),
    ]));
    let batch = |offset: i64| {
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![Some("Q1"), None, Some("Q3")])),
                Arc::new(ListArray::from_iter_primitive::<Int64Type, _, _>(vec![
                    Some(vec![Some(1), None]),
                    None,
                    Some(vec![]),
                ])),
                Arc::new(Int64Array::from(vec![offset, offset + 1, offset + 2])),
            ],
        )
        .unwrap()
    };
    let batches = vec![batch(0), batch(3), batch(6)];
    write_arrow_batches(path, schema.clone(), batches.clone(), Some(4)).unwrap();

    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap()).unwrap();
    let meta = reader.metadata().clone();
    let rows: Vec<usize> = meta.row_groups().iter().map(|g| g.num_rows() as usize).collect();
    assert_eq!(rows, vec![4, 4, 1]);
    // List items take the specification's name, as pyarrow writes them
    let leaf = meta.file_metadata().schema_descr().column(1).path().string();
    assert_eq!(leaf, "values.list.element");
    let read: Vec<RecordBatch> = reader.build().unwrap().map(|b| b.unwrap()).collect();
    let read = arrow::compute::concat_batches(&read[0].schema(), &read).unwrap();
    let written = arrow::compute::concat_batches(&schema, &batches).unwrap();
    // The same values; the list's item field is named `element` as read back
    for (r, w) in read.columns().iter().zip(written.columns()) {
        assert_eq!(r, &arrow::compute::cast(w, r.data_type()).unwrap());
    }
}
