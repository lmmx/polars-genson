//! Scratch benchmark (not for commit): genson's Parquet writer alone on the columns of a
//! Parquet file. The file is read into record batches of `batch_rows` rows, each batch's
//! columns wrapped as one struct, and written with `write_struct_columns` unnested, with
//! the layout of a target-schema write (spec list names). Prints wall and CPU time of the
//! write only.
//!
//! cargo run --release --features parquet --example write_bench -- <in.parquet> <out.parquet> [batch_rows]

use arrow::array::{Array, StructArray};
use genson_core::parquet::{write_struct_columns, StructLayout};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::time::Instant;

fn cpu_seconds() -> f64 {
    // utime and stime (fields 14 and 15 of /proc/self/stat), in clock ticks of 1/100 s
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let after = &stat[stat.rfind(')').unwrap() + 2..];
    let fields: Vec<&str> = after.split(' ').collect();
    let utime: f64 = fields[11].parse().unwrap();
    let stime: f64 = fields[12].parse().unwrap();
    (utime + stime) / 100.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let batch_rows: usize = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(8192);
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(input).unwrap())
        .unwrap()
        .with_batch_size(batch_rows)
        .build()
        .unwrap();
    let batches: Vec<_> = reader.map(|b| b.unwrap()).collect();
    let fields = batches[0].schema().fields().clone();
    let arrays: Vec<StructArray> = batches
        .into_iter()
        .map(|b| StructArray::new(fields.clone(), b.columns().to_vec(), None))
        .collect();
    let rows: usize = arrays.iter().map(|a| a.len()).sum();
    let (c0, t0) = (cpu_seconds(), Instant::now());
    write_struct_columns(
        output,
        "x",
        arrays,
        &fields,
        &[],
        None,
        StructLayout { unnest: true, spec_names: true },
    )
    .unwrap();
    let wall = t0.elapsed().as_secs_f64();
    println!(
        "{{\"rows\": {rows}, \"batch_rows\": {batch_rows}, \"write_s\": {wall:.3}, \"cpu_s\": {:.2}}}",
        cpu_seconds() - c0
    );
}
