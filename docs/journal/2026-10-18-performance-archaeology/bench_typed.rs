//! MEASUREMENT ONLY: normalisation into Arrow of an NDJSON file's rows, by the `Value` path
//! (`serde_json::from_str`, `NormalisePlan::normalise`, `values_to_struct_array`) and by
//! `ArrowPlan::normalise`, over the same batches (as `normalise_from_parquet` cuts them),
//! against the schema inferred from the rows. Asserts the arrays and pruned values are
//! equal, and prints each path's time per run (one thread, or the rayon pool with
//! THREADS=1). Options: MAP_THRESHOLD (default 20), WRAP=<field> (`wrap_root`),
//! PRUNE=<a,b> (prune names), ENCODING=kv|mapping (default kv), ONLY=value|typed (that
//! path alone, in `measured`, for callgrind's `--toggle-collect='*measured*'`).
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_typed rows.ndjson [runs]`
use genson_core::normalise::{prune_schema, MapEncoding, NormaliseConfig, NormalisePlan};
use genson_core::parquet::{avro_record_fields, values_to_struct_array};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use rayon::prelude::*;
use std::collections::HashSet;
use std::time::Instant;

/// `f`'s result and its time.
#[inline(never)]
fn measured<R>(f: impl FnOnce() -> R) -> (R, f64) {
    let t = Instant::now();
    let r = f();
    (r, t.elapsed().as_secs_f64())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args.get(2).map_or(3, |s| s.parse().unwrap());
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let rows: Vec<Option<&str>> = text.lines().map(Some).collect();
    let env = |k: &str| std::env::var(k).ok();
    let wrap = env("WRAP");
    let config = SchemaInferenceConfig {
        avro: true,
        map_threshold: env("MAP_THRESHOLD").map_or(20, |s| s.parse().unwrap()),
        wrap_root: wrap.clone(),
        ..Default::default()
    };
    let present: Vec<&str> = rows.iter().flatten().copied().collect();
    let schema = infer_json_schema_from_strings(&present, config).unwrap().schema;
    let prune: HashSet<String> = env("PRUNE")
        .map(|p| p.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    let mut out_schema = schema.clone();
    prune_schema(&mut out_schema, &prune);
    let native = env("ENCODING").as_deref() == Some("mapping");
    let fields = avro_record_fields(&out_schema, native).unwrap();
    let cfg = NormaliseConfig {
        map_encoding: if native { MapEncoding::Mapping } else { MapEncoding::KeyValueEntries },
        wrap_root: wrap,
        ..Default::default()
    };
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    let arrow_plan = plan.to_arrow(&fields).expect("the plan decodes into these fields");
    let threads = env("THREADS").is_some();
    let batch = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let by_value = |(b, chunk): (usize, &[Option<&str>])| {
        let mut pruned = Vec::new();
        let values: Vec<serde_json::Value> = chunk
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let Some(s) = s else { return serde_json::Value::Null };
                let v = serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
                let (normalised, mut p) = plan.normalise(v);
                p.iter_mut().for_each(|p| p.row = b * batch + i);
                pruned.extend(p);
                normalised
            })
            .collect();
        (values_to_struct_array(&values, &fields).unwrap(), pruned)
    };
    let typed = |(b, chunk): (usize, &[Option<&str>])| {
        arrow_plan.normalise(chunk, b * batch).expect("the plan decodes these rows")
    };
    let run_value = || -> Vec<_> {
        if threads {
            rows.par_chunks(batch).enumerate().map(by_value).collect()
        } else {
            rows.chunks(batch).enumerate().map(by_value).collect()
        }
    };
    let run_typed = || -> Vec<_> {
        if threads {
            rows.par_chunks(batch).enumerate().map(typed).collect()
        } else {
            rows.chunks(batch).enumerate().map(typed).collect()
        }
    };
    match env("ONLY").as_deref() {
        Some("value") => return println!("Value path {:.3}s", measured(run_value).1),
        Some("typed") => return println!("typed {:.3}s", measured(run_typed).1),
        _ => {}
    }
    for _ in 0..runs {
        let (old, t_old) = measured(run_value);
        let (new, t_new) = measured(run_typed);
        let mut pruned = 0;
        for (o, n) in old.iter().zip(&new) {
            assert_eq!(o.0, n.0, "arrays differ");
            assert_eq!(o.1, n.1, "pruned values differ");
            pruned += n.1.len();
        }
        println!(
            "{} rows, {:.1} MB: Value path {:.3}s, typed {:.3}s ({:.1}x); equal, {} pruned",
            rows.len(),
            text.len() as f64 / 1e6,
            t_old,
            t_new,
            t_old / t_new,
            pruned
        );
    }
}
