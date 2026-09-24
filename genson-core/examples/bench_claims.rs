//! Times `infer_json_schema_from_strings` on an NDJSON file (one row per line).
//!
//! Usage: bench_claims <file.ndjson> [runs]   (env MB overrides `max_builders`, default 100)
//! Prints wall time and a hash of the inferred schema, so a change can be checked for
//! byte-identical output. PROFILE=1 enables the per-chunk profile output.
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::hash::{Hash, Hasher};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let rows: Vec<String> = text.lines().map(str::to_owned).collect();
    let runs: usize = args.get(2).map_or(3, |s| s.parse().unwrap());
    let max_builders: usize = std::env::var("MB").map_or(100, |s| s.parse().unwrap());
    for _ in 0..runs {
        let cfg = SchemaInferenceConfig {
            delimiter: Some(b'\n'),
            max_builders: Some(max_builders),
            profile: std::env::var("PROFILE").is_ok(),
            ..Default::default()
        };
        let t = std::time::Instant::now();
        let res = infer_json_schema_from_strings(&rows, cfg).unwrap();
        let dt = t.elapsed();
        let s = res.schema.to_string();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        println!(
            "{:.2}s processed={} schema_len={} hash={:x}",
            dt.as_secs_f64(),
            res.processed_count,
            s.len(),
            h.finish()
        );
    }
}
