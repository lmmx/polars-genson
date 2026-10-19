//! MEASUREMENT ONLY: `normalise_from_parquet`'s typed rows on a Parquet chunk's claims
//! column, by the `Value` path (`serde_json::from_str`, `NormalisePlan::normalise`,
//! `values_to_struct_array`) and by `ArrowPlan::normalise`, over the same
//! batches, with wikidata-pq's claims options and prune names. Asserts the arrays and
//! pruned values are equal, and prints each path's time per run (one thread, or the rayon
//! pool with THREADS=1). SLIM=1 extracts wikidata-pq's label maps first, as its HF-copy
//! path does. ONLY=value or ONLY=typed runs that path alone, without the comparison, in
//! `measured` (for `valgrind --tool=callgrind --toggle-collect='*measured*'`, whose call
//! counts of mimalloc's `mi_malloc_aligned` and the like count its allocations).
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_typed_wpq chunk.parquet [runs]`
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

fn claims_config() -> SchemaInferenceConfig {
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
    SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 0,
        unify_maps: true,
        force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
        force_scalar_promotion: set(&["mainsnak", "datavalue", "precision", "latitude", "longitude"]),
        no_unify: set(&["qualifiers"]),
        wrap_root: Some("claims".to_string()),
        max_builders: Some(100),
        avro: true,
        ..Default::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args.get(2).map_or(3, |s| s.parse().unwrap());
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let read = column.rows();
    let slim;
    let rows: Vec<Option<&str>> = if std::env::var("SLIM").is_ok() {
        let spec: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
            .iter()
            .map(|(f, k)| (f.to_string(), k.to_string()))
            .collect();
        slim = genson_core::extract::extract_invariants_from(&read, &spec).unwrap().rows;
        slim.iter().map(Option::as_deref).collect()
    } else {
        read
    };
    let present: Vec<&str> = rows.iter().flatten().copied().collect();
    let bytes: usize = present.iter().map(|r| r.len()).sum();
    let schema = infer_json_schema_from_strings(&present, claims_config()).unwrap().schema;
    let prune: HashSet<String> = ["mainsnak__string", "value__string", "value", "error"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut out_schema = schema.clone();
    prune_schema(&mut out_schema, &prune);
    let fields = avro_record_fields(&out_schema, false).unwrap();
    let cfg = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root: Some("claims".to_string()),
    };
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    let threads = std::env::var("THREADS").is_ok();
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
    let arrow_plan = plan.to_arrow(&fields).expect("the plan decodes into these fields");
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
    match std::env::var("ONLY").as_deref() {
        Ok("value") => {
            println!("Value path {:.3}s", measured(run_value).1);
            return;
        }
        Ok("typed") => {
            println!("typed {:.3}s", measured(run_typed).1);
            return;
        }
        _ => {}
    }
    for _ in 0..runs {
        let (old, t_old) = measured(run_value);
        let (new, t_new) = measured(run_typed);
        assert_eq!(old.len(), new.len());
        let mut pruned = 0;
        for (o, n) in old.iter().zip(&new) {
            assert_eq!(o.0, n.0, "arrays differ");
            assert_eq!(o.1, n.1, "pruned values differ");
            pruned += n.1.len();
        }
        println!(
            "{} rows, {:.1} MB: Value path {:.3}s, typed {:.3}s ({:.1}x); equal, {} pruned",
            rows.len(),
            bytes as f64 / 1e6,
            t_old,
            t_new,
            t_old / t_new,
            pruned
        );
    }
}
