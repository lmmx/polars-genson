//! MEASUREMENT ONLY: inference's row phase on a Parquet chunk's claims column, stage by
//! stage, on one thread: serde validation, the `wrap_root` copy, simd-json's tape, the
//! row's builder (tape plus `add_object`), its hash, the merge of the distinct ones into
//! one builder (chunks of 100, as `max_builders`), and the final `to_schema`.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_rows_phase chunk.parquet`
use genson_core::genson_rs::{build_json_schema_into, get_builder, BuildConfig, SchemaBuilder};
use std::collections::HashSet;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let rows: Vec<&str> = column.rows().into_iter().flatten().collect();
    let t = Instant::now();
    for r in &rows {
        let mut de = serde_json::Deserializer::from_str(r);
        serde::de::IgnoredAny::deserialize(&mut de).unwrap();
        de.end().unwrap();
    }
    let t_validate = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let wrapped: Vec<Vec<u8>> = rows
        .iter()
        .map(|r| [&b"{\"claims\":"[..], r.as_bytes(), b"}"].concat())
        .collect();
    let t_wrap = t.elapsed().as_secs_f64();
    let t = Instant::now();
    for w in &wrapped {
        let mut b = w.clone();
        let tape = simd_json::to_tape(&mut b).unwrap();
        std::hint::black_box(&tape);
    }
    let t_tape = t.elapsed().as_secs_f64();
    let cfg = BuildConfig { delimiter: Some(b'\n'), ignore_outer_array: false };
    let t = Instant::now();
    let builders: Vec<SchemaBuilder> = wrapped
        .iter()
        .map(|w| {
            let mut b = get_builder(None);
            let mut bytes = w.clone();
            build_json_schema_into(&mut b, &mut bytes, &cfg);
            b
        })
        .collect();
    let t_build = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let hashes: Vec<u64> = builders.iter().map(SchemaBuilder::schema_hash).collect();
    let t_hash = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut main = get_builder(None);
    let mut seen = HashSet::new();
    let mut dups = Vec::new();
    let mut it = builders.into_iter().zip(hashes);
    loop {
        let chunk: Vec<_> = it.by_ref().take(100).collect();
        if chunk.is_empty() {
            break;
        }
        let mut distinct = Vec::new();
        for (b, h) in chunk {
            if seen.insert(h) { distinct.push(b) } else { dups.push(b) }
        }
        main.add_builders(distinct);
    }
    let t_merge = t.elapsed().as_secs_f64();
    let t = Instant::now();
    drop(dups);
    let t_drop = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let schema = main.to_schema();
    let t_schema = t.elapsed().as_secs_f64();
    let t = Instant::now();
    drop(main);
    let t_drop_main = t.elapsed().as_secs_f64();
    println!(
        "{} rows, {} distinct: validate {t_validate:.3} wrap {t_wrap:.3} tape {t_tape:.3} build {t_build:.3} (incl. tape) hash {t_hash:.3} merge {t_merge:.3} drop-dups {t_drop:.3} to_schema {t_schema:.3} drop-main {t_drop_main:.3}; schema {} bytes",
        rows.len(),
        seen.len(),
        schema.to_string().len()
    );
}
use serde::Deserialize;
