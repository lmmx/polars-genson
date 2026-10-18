//! MEASUREMENT ONLY (ceiling probe): decode an NDJSON corpus straight into typed Arrow
//! with arrow-json, one thread, no intermediate `serde_json::Value` trees. The Arrow
//! schema is arrow-json's own inference over the corpus (each row wrapped as
//! `{"claims": row}`), so the output shape is not genson's (no maps, no promoted scalars):
//! the timing bounds what one pass from JSON text to typed Arrow costs on this data.
//!
//! Copy to genson-core/examples/ and run with `--features parquet`:
//! `bench_arrow_json rows.ndjson`
use std::io::{BufReader, Cursor};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let wrapped: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| format!("{{\"claims\":{l}}}\n"))
        .collect();
    let t = Instant::now();
    let (schema, _) = arrow::json::reader::infer_json_schema(
        BufReader::new(Cursor::new(wrapped.as_bytes())),
        None,
    )
    .unwrap();
    println!("infer {:.3}s, {} top-level fields", t.elapsed().as_secs_f64(), schema.fields().len());
    let t = Instant::now();
    let mut decoder = arrow::json::ReaderBuilder::new(Arc::new(schema))
        .with_batch_size(1024)
        .build_decoder()
        .unwrap();
    let mut buf = wrapped.as_bytes();
    let mut rows = 0;
    while !buf.is_empty() {
        let n = decoder.decode(buf).unwrap();
        buf = &buf[n..];
        if let Some(batch) = decoder.flush().unwrap() {
            rows += batch.num_rows();
        }
    }
    if let Some(batch) = decoder.flush().unwrap() {
        rows += batch.num_rows();
    }
    println!("decode {:.3}s ({} rows, {} MB)", t.elapsed().as_secs_f64(), rows, wrapped.len() / 1_000_000);
}
