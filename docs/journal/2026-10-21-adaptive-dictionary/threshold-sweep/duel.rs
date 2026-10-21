//! Built as its own crate (not in the workspace), in release with the workspace's profile
//! (codegen-units 1, thin LTO, opt-level 2), from these dependencies, with genson-core a
//! checkout carrying `threshold_from_env.diff`, the lock taken from the workspace's:
//!
//!   arrow = { version = "60", default-features = false }
//!   genson-core = { path = "<checkout>/genson-core", features = ["parquet"] }
//!   libc = "0.2"
//!   parquet = { version = "60", default-features = false, features = ["arrow", "zstd"] }
//!   polars = { version = "0.55.2", default-features = false, features = ["lazy", "parquet", "dtype-full"] }
//!   polars-core = { version = "0.55.2", default-features = false }
//!   polars-parquet = { version = "0.55.2", default-features = false }
//!   rayon = "1"
//!
//! One write of a Parquet file's table by genson's writer or Polars', in one binary built
//! once (same toolchain, profile and allocator for both). The table is read into memory
//! first; only the write is timed.
//!
//!   duel genson         <in.parquet> <out.parquet>   genson-core `write_struct_columns`
//!   duel polars         <in.parquet> <out.parquet>   Polars sink, 1Mi-row row groups
//!   duel polars-default <in.parquet> <out.parquet>   Polars sink, its default row groups
//!
//! Both write zstd level 1. genson's row groups are arrow-rs's default of 1,048,576 rows,
//! which `polars` matches. genson's input arrives as batches of a sixteenth of the rows
//! (as `normalise_from_parquet` decodes them), Polars' as the frame its reader returns.
//! Prints {"writer", "rows", "wall", "user", "sys"}.

use arrow::array::{Array, StructArray};
use genson_core::parquet::{write_struct_columns, StructLayout};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use polars::prelude::*;
use std::fs::File;
use std::time::Instant;

// The allocator is genson-core's (mimalloc), for both writers.

const ROW_GROUP_ROWS: usize = 1024 * 1024;

fn usage() -> (f64, f64) {
    let mut r: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut r) };
    let s = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    (s(r.ru_utime), s(r.ru_stime))
}

fn timed(f: impl FnOnce()) -> (f64, f64, f64) {
    let (u0, s0) = usage();
    let t = Instant::now();
    f();
    let wall = t.elapsed().as_secs_f64();
    let (u1, s1) = usage();
    (wall, u1 - u0, s1 - s0)
}

fn genson(input: &str, output: &str) -> (usize, (f64, f64, f64)) {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(input).unwrap()).unwrap();
    let rows = builder.metadata().file_metadata().num_rows() as usize;
    let reader = builder.with_batch_size(rows.div_ceil(16).max(1)).build().unwrap();
    let batches: Vec<_> = reader.map(|b| b.unwrap()).collect();
    let fields = batches[0].schema().fields().clone();
    let arrays: Vec<StructArray> = batches
        .into_iter()
        .map(|b| StructArray::new(fields.clone(), b.columns().to_vec(), None))
        .collect();
    assert_eq!(arrays.iter().map(|a| a.len()).sum::<usize>(), rows);
    let layout = StructLayout { unnest: true, spec_names: true };
    let t = timed(|| {
        write_struct_columns(output, "x", arrays, &fields, &[], None, layout).unwrap();
    });
    (rows, t)
}

fn polars(input: &str, output: &str, row_group_size: Option<usize>) -> (usize, (f64, f64, f64)) {
    let df = ParquetReader::new(File::open(input).unwrap()).finish().unwrap();
    let rows = df.height();
    let options = ParquetWriteOptions {
        compression: ParquetCompression::Zstd(Some(polars_parquet::write::ZstdLevel::try_new(1).unwrap())),
        statistics: StatisticsOptions::default(),
        row_group_size,
        data_page_size: None,
        key_value_metadata: None,
        arrow_schema: None,
        compat_level: None,
    };
    let t = timed(|| {
        df.lazy()
            .sink(
                SinkDestination::File { target: SinkTarget::Path(PlRefPath::new(output)) },
                FileWriteFormat::Parquet(Arc::new(options)),
                UnifiedSinkArgs::default(),
            )
            .unwrap()
            .collect_with_engine(Engine::Streaming)
            .unwrap();
    });
    (rows, t)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (writer, input, output) = (args[1].as_str(), &args[2], &args[3]);
    // Start both writers' thread pools before anything is timed
    rayon::broadcast(|_| ());
    polars_core::runtime::THREAD_POOL.broadcast(|_| ());
    let (rows, (wall, user, sys)) = match writer {
        "genson" => genson(input, output),
        "polars" => polars(input, output, Some(ROW_GROUP_ROWS)),
        "polars-default" => polars(input, output, None),
        _ => panic!("unknown writer {writer}"),
    };
    println!(
        "{{\"writer\": \"{writer}\", \"rows\": {rows}, \"wall\": {wall:.4}, \"user\": {user:.4}, \"sys\": {sys:.4}}}"
    );
}
