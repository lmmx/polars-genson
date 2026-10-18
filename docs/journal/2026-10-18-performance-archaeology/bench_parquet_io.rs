//! MEASUREMENT ONLY: the Parquet ends of `normalise_from_parquet`, one thread. Times
//! `read_string_column`, and `read_columns` (the same column kept as Arrow), on an input
//! file, then (given an Avro schema) normalises every row as bench_normalise.rs does,
//! builds the typed struct array, and times `write_struct_column` and
//! `write_string_column` (of the normalised JSON) to a temporary path. Normalisation
//! itself is not timed here.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_parquet_io rows.parquet column avro.json out_dir`
use genson_core::normalise::{normalise_values_pruned, MapEncoding, NormaliseConfig};
use std::collections::HashSet;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, column, avro, out_dir) = (&args[1], &args[2], &args[3], &args[4]);
    let t = Instant::now();
    let rows = genson_core::parquet::read_string_column(input, column).unwrap();
    let read = t.elapsed();
    let bytes: usize = rows.iter().map(String::len).sum();
    // the same column kept as Arrow arrays (no String per row)
    let t = Instant::now();
    let columns = genson_core::parquet::read_columns(input, &[column.to_string()]).unwrap();
    let read_arrow = t.elapsed();
    drop(columns);
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(avro).unwrap()).unwrap();
    let cfg = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root: Some("claims".to_string()),
    };
    let prune = HashSet::new();
    let normalised: Vec<serde_json::Value> = rows
        .iter()
        .map(|s| {
            let v: serde_json::Value = serde_json::from_str(s).unwrap();
            normalise_values_pruned(vec![v], &schema, &cfg, &prune).0.pop().unwrap()
        })
        .collect();
    let fields = genson_core::parquet::avro_record_fields(&schema, false).unwrap();
    let array = genson_core::parquet::values_to_struct_array(&normalised, &fields).unwrap();
    let t = Instant::now();
    let typed = format!("{out_dir}/typed.parquet");
    genson_core::parquet::write_struct_column(&typed, "claims", vec![array], &fields, None).unwrap();
    let write_typed = t.elapsed();
    let text: Vec<String> = normalised.iter().map(|v| serde_json::to_string(v).unwrap()).collect();
    let text_bytes: usize = text.iter().map(String::len).sum();
    let t = Instant::now();
    let strings = format!("{out_dir}/strings.parquet");
    genson_core::parquet::write_string_column(&strings, "claims", text, None).unwrap();
    let write_strings = t.elapsed();
    let size = |p: &str| std::fs::metadata(p).unwrap().len() / 1_000_000;
    println!(
        "read {:.3}s ({} rows, {} MB of JSON from {} MB) read_columns {:.3}s write_struct {:.3}s ({} MB) write_string {:.3}s ({} MB of JSON to {} MB)",
        read.as_secs_f64(),
        rows.len(),
        bytes / 1_000_000,
        size(input),
        read_arrow.as_secs_f64(),
        write_typed.as_secs_f64(),
        size(&typed),
        write_strings.as_secs_f64(),
        text_bytes / 1_000_000,
        size(&strings),
    );
}
