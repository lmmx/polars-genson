//! MEASUREMENT ONLY (research/profile, not for the repository): wikidata-pq's per-column
//! genson calls (src/wikidata/process.py) on a Parquet chunk, split into sub-steps, each in
//! a function never inlined so callgrind's inclusive cost names it.
//!
//! Usage: prof3 <chunk.parquet> <preset> <mode> [runs]
//!   preset: sitelinks (forced map, as normalise_sitelinks) | sitelinks_unify (map_threshold
//!           0 + unify_maps, no forcing) | labels | descriptions | aliases | claims | lookup
//!   mode:   infer | json (infer, then normalise_to_json as the normalise_json expression)
//!           | typed (infer, then the ArrowPlan batches, as normalise_json(decode=True) and
//!           normalise_from_parquet(typed=True)) | full (claims: extract, infer, typed)
//! THREADS=n runs in an n-thread pool (default: rayon's). With the `alloc-stats` feature
//! genson's counting allocator reports allocations, bytes and peak live bytes per sub-step.
//! Prints one JSON line per run.
use genson_core::normalise::{normalise_to_json, MapEncoding, NormaliseConfig, NormalisePlan};
use genson_core::parquet::avro_record_fields;
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use rayon::prelude::*;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

fn rusage() -> (f64, i64) {
    let mut ru: [i64; 18] = [0; 18];
    extern "C" {
        fn getrusage(who: i32, usage: *mut [i64; 18]) -> i32;
    }
    unsafe { getrusage(0, &mut ru) };
    ((ru[0] + ru[2]) as f64 + (ru[1] + ru[3]) as f64 / 1e6, ru[4])
}

struct Steps(Vec<String>);
impl Steps {
    fn step<R>(&mut self, name: &str, f: impl FnOnce() -> R) -> R {
        #[cfg(feature = "alloc-stats")]
        let a0 = genson_core::phase::counters(true);
        let (c0, _) = rusage();
        let t = std::time::Instant::now();
        let r = f();
        let wall = t.elapsed().as_secs_f64();
        let cpu = rusage().0 - c0;
        #[allow(unused_mut)]
        let mut extra = String::new();
        #[cfg(feature = "alloc-stats")]
        {
            let a1 = genson_core::phase::counters(false);
            extra = format!(
                ",\"allocs\":{},\"bytes\":{},\"live_before\":{},\"peak_live\":{}",
                a1.0 - a0.0,
                a1.1 - a0.1,
                a0.2,
                a1.3
            );
        }
        self.0.push(format!(
            "\"{name}\":{{\"wall\":{wall:.6},\"cpu\":{cpu:.6}{extra}}}"
        ));
        r
    }
}

fn config(preset: &str) -> SchemaInferenceConfig {
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
    let base = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        avro: true,
        profile: std::env::var("PROFILE").is_ok(),
        ..Default::default()
    };
    match preset {
        "sitelinks" => SchemaInferenceConfig {
            wrap_root: Some("sitelinks".into()),
            force_field_types: [("sitelinks".to_string(), "map".to_string())].into(),
            max_builders: Some(100),
            ..base
        },
        "sitelinks_unify" => SchemaInferenceConfig {
            wrap_root: Some("sitelinks".into()),
            map_threshold: 0,
            unify_maps: true,
            max_builders: Some(100),
            ..base
        },
        "labels" | "descriptions" | "aliases" => SchemaInferenceConfig {
            wrap_root: Some(preset.into()),
            map_threshold: 0,
            ..base
        },
        "lookup" => SchemaInferenceConfig {
            wrap_root: Some("labels".into()),
            map_threshold: 0,
            ..base
        },
        "claims" => SchemaInferenceConfig {
            map_threshold: 0,
            unify_maps: true,
            force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
            force_scalar_promotion: set(&["mainsnak", "datavalue", "precision", "latitude", "longitude"]),
            no_unify: set(&["qualifiers"]),
            wrap_root: Some("claims".into()),
            max_builders: Some(100),
            ..base
        },
        _ => panic!("unknown preset {preset}"),
    }
}

fn column_of(preset: &str) -> &str {
    match preset {
        "sitelinks" | "sitelinks_unify" => "sitelinks",
        "lookup" => "value",
        p => p,
    }
}

#[inline(never)]
fn step_extract(rows: &[Option<&str>]) -> genson_core::extract::Extracted {
    let spec: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
        .iter()
        .map(|(f, k)| (f.to_string(), k.to_string()))
        .collect();
    genson_core::extract::extract_invariants_from(rows, &spec).unwrap()
}

#[inline(never)]
fn step_infer(present: &[&str], cfg: SchemaInferenceConfig) -> genson_core::SchemaInferenceResult {
    infer_json_schema_from_strings(present, cfg).unwrap()
}

#[inline(never)]
fn step_json(plan: &NormalisePlan<'_>, fields: Option<&arrow::datatypes::Fields>, rows: &[Option<&str>]) -> Vec<String> {
    normalise_to_json(plan, fields, rows)
}

#[inline(never)]
fn step_typed(plan: &NormalisePlan<'_>, fields: &arrow::datatypes::Fields, rows: &[Option<&str>]) -> (usize, usize) {
    let arrow_plan = plan.to_arrow(fields);
    let batch_rows = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let out: Vec<(usize, bool)> = rows
        .par_chunks(batch_rows)
        .enumerate()
        .map(|(b, chunk)| {
            match arrow_plan.as_ref().and_then(|p| p.normalise(chunk, b * batch_rows)) {
                Some((array, _)) => (arrow::array::Array::len(&array), true),
                None => {
                    let values: Vec<serde_json::Value> = chunk
                        .iter()
                        .map(|s| {
                            let v = s.and_then(|s| serde_json::from_str(s).ok()).unwrap_or(serde_json::Value::Null);
                            plan.normalise(v).0
                        })
                        .collect();
                    let a = genson_core::parquet::values_to_struct_array(&values, fields).unwrap();
                    (arrow::array::Array::len(&a), false)
                }
            }
        })
        .collect();
    (out.iter().map(|o| o.0).sum(), out.iter().filter(|o| !o.1).count())
}

#[inline(never)]
fn measured(rows: &[Option<&str>], preset: &str, mode: &str, steps: &mut Steps) -> String {
    let slim;
    let rows: Vec<Option<&str>> = if mode == "full" {
        slim = steps.step("extract", || step_extract(rows));
        slim.rows.iter().map(Option::as_deref).collect()
    } else {
        rows.to_vec()
    };
    let present: Vec<&str> = rows.iter().flatten().copied().filter(|s| !s.trim().is_empty()).collect();
    let mut cfg = config(preset);
    if std::env::var("P3_JSON").is_ok() {
        cfg.avro = false;
    }
    let wrap_root = cfg.wrap_root.clone();
    let res = steps.step("infer", || step_infer(&present, cfg));
    let mut h = std::collections::hash_map::DefaultHasher::new();
    res.schema.to_string().hash(&mut h);
    if let Ok(path) = std::env::var("P3_SCHEMA_OUT") {
        std::fs::write(path, serde_json::to_string_pretty(&res.schema).unwrap()).unwrap();
    }
    let mut info = format!("\"schema_hash\":\"{:016x}\",\"processed\":{}", h.finish(), res.processed_count);
    if mode == "infer" {
        return info;
    }
    let ncfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root,
        ..Default::default()
    };
    let prune: HashSet<String> = if preset == "claims" {
        ["mainsnak__string", "value__string", "value", "error"].iter().map(|s| s.to_string()).collect()
    } else {
        HashSet::new()
    };
    let (plan, fields) = steps.step("plan", || {
        let plan = NormalisePlan::new(&res.schema, &ncfg, &prune);
        let fields = avro_record_fields(&res.schema, false).ok();
        (plan, fields)
    });
    if mode == "json" {
        let out = steps.step("normalise_to_json", || step_json(&plan, fields.as_ref(), &rows));
        let bytes: usize = out.iter().map(String::len).sum();
        info += &format!(",\"json_bytes\":{bytes}");
        steps.step("drop_json", || drop(out));
    } else if mode == "typed_concat" {
        let fields = fields.as_ref().unwrap();
        let arrow_plan = plan.to_arrow(fields).unwrap();
        let batch_rows = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
        let arrays: Vec<arrow::array::StructArray> = steps.step("typed", || {
            rows.par_chunks(batch_rows)
                .enumerate()
                .map(|(b, chunk)| arrow_plan.normalise(chunk, b * batch_rows).unwrap().0)
                .collect()
        });
        let all = steps.step("concat", || {
            let parts: Vec<&dyn arrow::array::Array> = arrays.iter().map(|a| a as &dyn arrow::array::Array).collect();
            arrow::compute::concat(&parts).unwrap()
        });
        info += &format!(",\"concat_bytes\":{}", arrow::array::Array::get_array_memory_size(&all));
    } else {
        let (n, fallback) = steps.step("typed", || step_typed(&plan, fields.as_ref().unwrap(), &rows));
        info += &format!(",\"typed_rows\":{n},\"fallback_batches\":{fallback}");
    }
    info
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, preset, mode) = (&args[1], args[2].as_str(), args[3].as_str());
    let runs: usize = args.get(4).map_or(1, |s| s.parse().unwrap());
    let column = genson_core::parquet::read_string_column_arrays(path, column_of(preset)).unwrap();
    let rows = column.rows();
    let bytes: usize = rows.iter().flatten().map(|r| r.len()).sum();
    let mut b = rayon::ThreadPoolBuilder::new();
    if let Ok(t) = std::env::var("THREADS") {
        b = b.num_threads(t.parse().unwrap());
    }
    let pool = b.build().unwrap();
    for run in 0..runs {
        let mut steps = Steps(Vec::new());
        let (c0, _) = rusage();
        let t = std::time::Instant::now();
        let info = pool.install(|| measured(&rows, preset, mode, &mut steps));
        let wall = t.elapsed().as_secs_f64();
        let (c1, rss) = rusage();
        println!(
            "{{\"run\":{run},\"preset\":\"{preset}\",\"mode\":\"{mode}\",\"rows\":{},\"bytes\":{bytes},\"wall\":{wall:.6},\"cpu\":{:.6},\"maxrss_kb\":{rss},{info},\"steps\":{{{}}}}}",
            rows.len(),
            c1 - c0,
            steps.0.join(",")
        );
    }
}
