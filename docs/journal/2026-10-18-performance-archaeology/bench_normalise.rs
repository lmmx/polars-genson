//! MEASUREMENT ONLY: the row loop of polars-genson-py's `normalise_from_parquet`, on one
//! thread, against a given Avro schema: per row `serde_json::from_str`, then
//! `normalise_values_pruned`, then either `serde_json::to_string` (string output) or, with
//! TYPED=1, `values_to_struct_array` over all rows (typed output). Prints each stage's time.
//! MAPPING=1 writes maps as objects (the `mapping` encoding) instead of key/value lists;
//! OUT=path writes the normalised rows as NDJSON.
//!
//! Copy to genson-core/examples/ and run (needs the `parquet` feature):
//! `cargo run --release -p genson-core --features parquet --example bench_normalise -- rows.ndjson avro.json`
use genson_core::normalise::{normalise_values_pruned, MapEncoding, NormaliseConfig};
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rows: Vec<String> = std::fs::read_to_string(&args[1])
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let cfg = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: if std::env::var("MAPPING").is_ok() {
            MapEncoding::Mapping
        } else {
            MapEncoding::KeyValueEntries
        },
        wrap_root: Some("claims".to_string()),
    };
    let prune = HashSet::new();
    let (mut parse, mut norm, mut ser) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let mut out_bytes = 0usize;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut normalised = Vec::with_capacity(rows.len());
    let mut dump = std::env::var("OUT").ok().map(|p| std::fs::File::create(p).unwrap());
    for s in &rows {
        let t = Instant::now();
        let v: serde_json::Value = serde_json::from_str(s).unwrap();
        parse += t.elapsed();
        let t = Instant::now();
        let (mut out, _) = normalise_values_pruned(vec![v], &schema, &cfg, &prune);
        let n = out.pop().unwrap();
        norm += t.elapsed();
        let t = Instant::now();
        let text = serde_json::to_string(&n).unwrap();
        std::hash::Hash::hash(&text, &mut hasher);
        out_bytes += text.len();
        if let Some(f) = dump.as_mut() {
            use std::io::Write;
            writeln!(f, "{text}").unwrap();
        }
        ser += t.elapsed();
        normalised.push(n);
    }
    println!(
        "rows {} parse {:.3}s normalise {:.3}s serialise {:.3}s (output {} MB, hash {:x})",
        rows.len(),
        parse.as_secs_f64(),
        norm.as_secs_f64(),
        ser.as_secs_f64(),
        out_bytes / 1_000_000,
        std::hash::Hasher::finish(&hasher)
    );
    if std::env::var("TYPED").is_ok() {
        let fields =
            genson_core::parquet::avro_record_fields(&schema, std::env::var("MAPPING").is_ok())
                .unwrap();
        let t = Instant::now();
        let array = genson_core::parquet::values_to_struct_array(&normalised, &fields).unwrap();
        println!("values_to_struct_array {:.3}s ({} rows)", t.elapsed().as_secs_f64(), arrow::array::Array::len(&array));
    }
}
