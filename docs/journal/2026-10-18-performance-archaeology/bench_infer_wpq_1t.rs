//! MEASUREMENT ONLY, in a one-thread rayon pool (so that every stage runs on the thread
//! callgrind follows; count from `--toggle-collect=genson_core::schema::infer_json_schema_from_strings*`):
//! `infer_json_schema_from_strings` on a Parquet chunk's claims column with
//! wikidata-pq's claims options (`CLAIMS_INFERENCE_OPTIONS`, `max_builders=100`,
//! `wrap_root="claims"`), as `normalise_claims_direct` runs it; with `SLIM=1`, on the rows
//! with wikidata-pq's label maps extracted first. Prints wall and CPU time and a hash of the
//! schema, per run. `PROFILE=1` prints genson's profile lines.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_infer_wpq chunk.parquet [runs]`
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::hash::{Hash, Hasher};

fn claims_config() -> SchemaInferenceConfig {
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
    SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 0,
        unify_maps: true,
        force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
        force_scalar_promotion: set(&[
            "mainsnak",
            "datavalue",
            "precision",
            "latitude",
            "longitude",
        ]),
        no_unify: set(&["qualifiers"]),
        wrap_root: Some("claims".to_string()),
        max_builders: Some(100),
        avro: true,
        profile: std::env::var("PROFILE").is_ok(),
        ..Default::default()
    }
}

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
    let runs: usize = args.get(2).map_or(5, |s| s.parse().unwrap());
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let rows = column.rows();
    let slim;
    let present: Vec<&str> = if std::env::var("SLIM").is_ok() {
        let spec: Vec<(String, String)> = [
            ("labels", "id"),
            ("property-labels", "property"),
            ("unit-labels", "unit"),
        ]
        .iter()
        .map(|(f, k)| (f.to_string(), k.to_string()))
        .collect();
        slim = genson_core::extract::extract_invariants_from(&rows, &spec)
            .unwrap()
            .rows;
        slim.iter().flatten().map(String::as_str).collect()
    } else {
        rows.iter().flatten().copied().collect()
    };
    let bytes: usize = present.iter().map(|r| r.len()).sum();
    for _ in 0..runs {
        let cpu = cpu_seconds();
        let t = std::time::Instant::now();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let res = pool
            .install(|| infer_json_schema_from_strings(&present, claims_config()))
            .unwrap();
        let dt = t.elapsed().as_secs_f64();
        let cpu = cpu_seconds() - cpu;
        let s = res.schema.to_string();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        println!(
            "{} rows, {:.1} MB: {:.3}s cpu={:.3}s processed={} hash={:x}",
            present.len(),
            bytes as f64 / 1e6,
            dt,
            cpu,
            res.processed_count,
            h.finish()
        );
    }
}
