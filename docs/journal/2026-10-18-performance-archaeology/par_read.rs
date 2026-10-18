//! MEASUREMENT ONLY: one column decoded by one reader, or by 4, 8 or 16 over disjoint row
//! ranges, with the offset index loaded or not; medians of 5 runs.
//!
//! Copy to genson-core/examples/ and run with `--features parquet`:
//! `par_read chunk.parquet claims`
use parquet::arrow::arrow_reader::{ArrowReaderOptions, ParquetRecordBatchReaderBuilder, RowSelection, RowSelector};
use parquet::arrow::ProjectionMask;
use parquet::file::metadata::PageIndexPolicy;
use rayon::prelude::*;
use std::fs::File;
use std::time::Instant;

fn read(path: &str, column: &str, range: Option<(usize, usize, usize)>, page_index: bool) -> usize {
    let policy = if page_index { PageIndexPolicy::Optional } else { PageIndexPolicy::Skip };
    let opts = ArrowReaderOptions::new().with_offset_index_policy(policy);
    let builder = ParquetRecordBatchReaderBuilder::try_new_with_options(File::open(path).unwrap(), opts).unwrap();
    let idx = builder.schema().index_of(column).unwrap();
    let mask = ProjectionMask::roots(builder.parquet_schema(), [idx]);
    let mut builder = builder.with_projection(mask);
    if let Some((skip, take, total)) = range {
        let mut sel = vec![];
        if skip > 0 { sel.push(RowSelector::skip(skip)); }
        sel.push(RowSelector::select(take));
        if skip + take < total { sel.push(RowSelector::skip(total - skip - take)); }
        builder = builder.with_row_selection(RowSelection::from(sel));
    }
    builder.build().unwrap().map(|b| b.unwrap().num_rows()).sum()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, column) = (&args[1], &args[2]);
    let total = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap()).unwrap().metadata().file_metadata().num_rows() as usize;
    for page_index in [false, true] {
        for parts in [1usize, 4, 8, 16] {
            let mut ts = vec![];
            for _ in 0..5 {
                let t = Instant::now();
                let n: usize = (0..parts).into_par_iter().map(|p| {
                    let from = total * p / parts; let to = total * (p + 1) / parts;
                    read(path, column, (parts > 1).then_some((from, to - from, total)), page_index)
                }).sum();
                assert_eq!(n, total);
                ts.push(t.elapsed().as_secs_f64());
            }
            ts.sort_by(f64::total_cmp);
            println!("page_index={page_index} parts={parts:2}: median {:.3}s min {:.3}s", ts[2], ts[0]);
        }
    }
}
