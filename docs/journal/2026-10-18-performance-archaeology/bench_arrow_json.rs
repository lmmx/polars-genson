//! MEASUREMENT ONLY: decode normalised NDJSON (bench_normalise.rs with MAPPING=1 OUT=...)
//! into typed Arrow with arrow-json, one thread, against the Arrow schema genson derives
//! from the Avro schema (`avro_record_fields`, native maps): a production-grade one-pass
//! decoder from JSON text to Arrow, on the normalised data's shape.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_arrow_json normalised.ndjson avro.json`
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read(&args[1]).unwrap();
    let avro: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let fields = genson_core::parquet::avro_record_fields(&avro, true).unwrap();
    let schema = Arc::new(arrow::datatypes::Schema::new(fields));
    let t = Instant::now();
    let mut decoder = arrow::json::ReaderBuilder::new(schema)
        .with_batch_size(64)
        .build_decoder()
        .unwrap();
    let mut buf = &text[..];
    let mut rows = 0;
    loop {
        let n = decoder.decode(buf).unwrap();
        buf = &buf[n..];
        if let Some(batch) = decoder.flush().unwrap() {
            rows += batch.num_rows();
        }
        if buf.is_empty() {
            break;
        }
    }
    if let Some(batch) = decoder.flush().unwrap() {
        rows += batch.num_rows();
    }
    println!("arrow-json decode {:.3}s ({} rows, {} MB)", t.elapsed().as_secs_f64(), rows, text.len() / 1_000_000);
}
