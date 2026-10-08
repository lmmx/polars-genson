//! Times `infer_json_schema_from_strings` on an NDJSON file (one row per line).
//!
//! Usage: bench_claims <file.ndjson> [runs]   (env MB overrides `max_builders`, default 100;
//! ROWS limits the rows read; WIKIDATA=1 adds the Wikidata claims options `map_threshold=0`,
//! `unify_maps`, `wrap_root="claims"`)
//! Prints wall time, CPU time (user + system) and a hash of the inferred schema, so a change
//! can be checked for byte-identical output. PROFILE=1 enables the per-chunk profile output.
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::hash::{Hash, Hasher};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    use std::io::BufRead;
    let file = std::io::BufReader::new(std::fs::File::open(&args[1]).unwrap());
    let limit: usize = std::env::var("ROWS").map_or(usize::MAX, |s| s.parse().unwrap());
    let rows: Vec<String> = file.lines().take(limit).map(Result::unwrap).collect();
    let runs: usize = args.get(2).map_or(3, |s| s.parse().unwrap());
    let max_builders: usize = std::env::var("MB").map_or(100, |s| s.parse().unwrap());
    for _ in 0..runs {
        let cfg = SchemaInferenceConfig {
            delimiter: Some(b'\n'),
            max_builders: Some(max_builders),
            profile: std::env::var("PROFILE").is_ok(),
            ..Default::default()
        };
        let cfg = if std::env::var("WIKIDATA").is_ok() {
            SchemaInferenceConfig {
                map_threshold: 0,
                unify_maps: true,
                wrap_root: Some("claims".to_string()),
                ..cfg
            }
        } else {
            cfg
        };
        let cpu = cpu_seconds();
        let t = std::time::Instant::now();
        let res = infer_json_schema_from_strings(&rows, cfg).unwrap();
        let dt = t.elapsed();
        let cpu = cpu_seconds() - cpu;
        let s = res.schema.to_string();
        if let Ok(path) = std::env::var("DUMP") {
            std::fs::write(path, &s).unwrap();
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        println!(
            "{:.2}s cpu={:.2}s processed={} schema_len={} hash={:x}",
            dt.as_secs_f64(),
            cpu,
            res.processed_count,
            s.len(),
            h.finish()
        );
    }
    let mut ru: Rusage = unsafe { std::mem::zeroed() };
    unsafe { getrusage(0, &mut ru) };
    println!("peak_rss={:.2}GB", ru.ru_maxrss as f64 / 1e6);
}

/// User plus system CPU time of the process so far, in seconds.
fn cpu_seconds() -> f64 {
    let mut ru: Rusage = unsafe { std::mem::zeroed() };
    unsafe { getrusage(0, &mut ru) };
    let secs = |tv: [i64; 2]| tv[0] as f64 + tv[1] as f64 / 1e6;
    secs(ru.ru_utime) + secs(ru.ru_stime)
}

#[repr(C)]
struct Rusage {
    ru_utime: [i64; 2],
    ru_stime: [i64; 2],
    ru_maxrss: i64,
    _rest: [i64; 13],
}

extern "C" {
    fn getrusage(who: i32, usage: *mut Rusage) -> i32;
}
