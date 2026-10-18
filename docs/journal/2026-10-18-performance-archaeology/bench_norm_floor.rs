//! MEASUREMENT ONLY (floor estimate for normalisation): per row, one thread, parse with
//! simd-json's tape and walk it once, appending every string (keys and values) to an
//! Arrow `StringBuilder` and counting the other values. A normaliser writing straight to
//! Arrow has to do at least this much: the timing is a lower bound for one pass from JSON
//! text to typed Arrow on this data, not a normaliser.
//!
//! Copy to genson-core/examples/ and run with `--features parquet`:
//! `bench_norm_floor rows.ndjson`
use simd_json::prelude::*;
use std::time::{Duration, Instant};

fn walk(v: simd_json::tape::Value, out: &mut arrow::array::StringBuilder, others: &mut usize) {
    if let Some(s) = v.as_str() {
        out.append_value(s);
    } else if let Some(o) = v.as_object() {
        for (k, x) in o.iter() {
            out.append_value(k);
            walk(x, out, others);
        }
    } else if let Some(a) = v.as_array() {
        for x in a.iter() {
            walk(x, out, others);
        }
    } else {
        *others += 1;
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let rows: Vec<Vec<u8>> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| format!("{{\"claims\":{l}}}").into_bytes())
        .collect();
    let (mut parse, mut append) = (Duration::ZERO, Duration::ZERO);
    let mut builder = arrow::array::StringBuilder::new();
    let mut others = 0;
    for row in &rows {
        let mut buf = row.clone();
        let t = Instant::now();
        let tape = simd_json::to_tape(&mut buf).unwrap();
        parse += t.elapsed();
        let t = Instant::now();
        walk(tape.as_value(), &mut builder, &mut others);
        append += t.elapsed();
    }
    let strings = arrow::array::ArrayBuilder::len(&builder);
    let t = Instant::now();
    let array = builder.finish();
    let finish = t.elapsed();
    println!(
        "rows {} tape parse {:.3}s walk+append {:.3}s finish {:.3}s ({} strings, {} other values, {} MB of string data)",
        rows.len(),
        parse.as_secs_f64(),
        append.as_secs_f64(),
        finish.as_secs_f64(),
        strings,
        others,
        arrow::array::Array::get_buffer_memory_size(&array) / 1_000_000
    );
}
