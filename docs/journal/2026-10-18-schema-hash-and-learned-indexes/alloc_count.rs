//! MEASUREMENT ONLY (needs the counting allocator of count_alloc.py): allocations and
//! requested bytes per stage on one thread over one pass, as in stage_split.rs, and for one
//! `infer_json_schema_from_strings` call (bench_claims options; WIKIDATA=1 for the claims ones).
use genson_core::genson_rs::{SchemaBuilder, ALLOCS, ALLOC_BYTES};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::hash::{Hash, Hasher};
use std::sync::atomic::Ordering::Relaxed;

fn snap() -> (u64, u64) {
    (ALLOCS.load(Relaxed), ALLOC_BYTES.load(Relaxed))
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<String> = text.lines().map(String::from).collect();
    let mut stages = [(0u64, 0u64); 4];
    for line in &lines {
        let mut buf = format!("{{\"claims\":{line}}}").into_bytes();
        let a = snap();
        let tape = simd_json::to_tape(&mut buf).unwrap();
        let b = snap();
        let mut builder = SchemaBuilder::new(None);
        builder.add_object(tape.as_value());
        let c = snap();
        std::hint::black_box(builder.schema_hash());
        let d = snap();
        drop(builder);
        let e = snap();
        for (i, (x, y)) in [(a, b), (b, c), (c, d), (d, e)].into_iter().enumerate() {
            stages[i].0 += y.0 - x.0;
            stages[i].1 += y.1 - x.1;
        }
    }
    for (name, (n, bytes)) in ["to_tape", "add_object", "schema_hash", "drop"].iter().zip(stages) {
        println!("stage {name} allocs {n} bytes {bytes}");
    }
    let cfg = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        max_builders: Some(100),
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
    let a = snap();
    let res = infer_json_schema_from_strings(&lines, cfg).unwrap();
    let b = snap();
    let s = res.schema.to_string();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    println!("e2e allocs {} bytes {} hash={:x}", b.0 - a.0, b.1 - a.1, h.finish());
}
