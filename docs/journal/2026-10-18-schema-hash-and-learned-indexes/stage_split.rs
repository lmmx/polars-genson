//! Single-threaded split of per-row CPU between simd-json parsing, building the row's nodes,
//! hashing the builder (`SchemaBuilder::schema_hash`) and dropping it, over two passes of an
//! NDJSON file, each row wrapped as `{"claims": row}`.
//!
//! Copy to genson-core/examples/ and run:
//! `cargo run --release -p genson-core --example stage_split -- rows200.ndjson`
use genson_core::genson_rs::SchemaBuilder;
use std::time::{Duration, Instant};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    let rows: Vec<Vec<u8>> = text
        .lines()
        .map(|l| format!("{{\"claims\":{l}}}").into_bytes())
        .collect();
    let (mut parse, mut build, mut hash, mut drop_t) = (Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let mut n_tape = 0usize;
    for _ in 0..2 {
        for row in &rows {
            let mut buf = row.clone();
            let t = Instant::now();
            let tape = simd_json::to_tape(&mut buf).unwrap();
            parse += t.elapsed();
            n_tape += tape.0.len();
            let t = Instant::now();
            let mut b = SchemaBuilder::new(None);
            b.add_object(tape.as_value());
            build += t.elapsed();
            let t = Instant::now();
            std::hint::black_box(b.schema_hash());
            hash += t.elapsed();
            let t = Instant::now();
            drop(b);
            drop_t += t.elapsed();
        }
    }
    let total = parse + build + hash + drop_t;
    let pct = |d: Duration| 100.0 * d.as_secs_f64() / total.as_secs_f64();
    println!("tape nodes per pass: {}", n_tape / 2);
    println!("parse  {:>8.3}s {:5.1}%", parse.as_secs_f64() / 2.0, pct(parse));
    println!("build  {:>8.3}s {:5.1}%", build.as_secs_f64() / 2.0, pct(build));
    println!("hash   {:>8.3}s {:5.1}%", hash.as_secs_f64() / 2.0, pct(hash));
    println!("drop   {:>8.3}s {:5.1}%", drop_t.as_secs_f64() / 2.0, pct(drop_t));
}
