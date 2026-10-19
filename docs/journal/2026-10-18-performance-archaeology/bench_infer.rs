//! MEASUREMENT ONLY: `infer_json_schema_from_strings` on an NDJSON file's rows, one string
//! per row, with the default options (Avro output) or: MAP_THRESHOLD, WRAP=<field>
//! (`wrap_root`), NDJSON=1 (`delimiter` b'\n'), UNIFY=1 (`unify_maps`). Prints wall and CPU
//! time, the processed count and a hash of the schema, per run; `PROFILE=1` prints genson's
//! profile lines.
//!
//! Copy to genson-core/examples/ and run with `--features avro`:
//! `bench_infer rows.ndjson [runs]`
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::hash::{Hash, Hasher};

fn cpu_seconds() -> f64 {
    let mut ru: [i64; 18] = [0; 18];
    extern "C" {
        fn getrusage(who: i32, usage: *mut [i64; 18]) -> i32;
    }
    unsafe { getrusage(0, &mut ru) };
    (ru[0] + ru[2]) as f64 + (ru[1] + ru[3]) as f64 / 1e6
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args.get(2).map_or(3, |s| s.parse().unwrap());
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let rows: Vec<&str> = text.lines().collect();
    let env = |k: &str| std::env::var(k).ok();
    for _ in 0..runs {
        let config = SchemaInferenceConfig {
            avro: true,
            map_threshold: env("MAP_THRESHOLD").map_or(20, |s| s.parse().unwrap()),
            wrap_root: env("WRAP"),
            delimiter: env("NDJSON").map(|_| b'\n'),
            unify_maps: env("UNIFY").is_some(),
            profile: env("PROFILE").is_some(),
            ..Default::default()
        };
        let cpu = cpu_seconds();
        let t = std::time::Instant::now();
        let res = infer_json_schema_from_strings(&rows, config).unwrap();
        let dt = t.elapsed().as_secs_f64();
        let cpu = cpu_seconds() - cpu;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        res.schema.to_string().hash(&mut h);
        println!(
            "{} rows, {:.1} MB: {dt:.3}s cpu={cpu:.3}s processed={} hash={:x}",
            rows.len(),
            text.len() as f64 / 1e6,
            res.processed_count,
            h.finish()
        );
    }
}
