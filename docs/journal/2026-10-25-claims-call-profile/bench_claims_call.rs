//! wikidata-pq's claims call (`normalise_from_parquet` with `avro_schema`, a target
//! `schema`, `extract_invariants`, `prune`, `output_format="arrow"`, `unnest`), stage by
//! stage as polars-genson-py runs it, for profiling: read, extract, decode, lookup write,
//! conform, record batches.
//!
//! Usage: bench_claims_call <chunk.parquet> <claims.avsc> <claims.spec.json> <runs> [only]
//! `only`: a stage name, to repeat the earlier stages once and that stage `runs` times.

use arrow::array::ArrayRef;
use genson_core::conform::{conform, data_type_from_spec};
use genson_core::extract::{extract_invariants_rows, Extracted, Scan};
use genson_core::normalise::{prune_schema, MapEncoding, NormaliseConfig, NormalisePlan};
use genson_core::parquet::{
    avro_record_fields, read_columns, read_string_column_arrays, struct_batches, write_lookup_table,
};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[repr(C)]
struct Rusage {
    utime: [i64; 2],
    stime: [i64; 2],
    rest: [i64; 14],
}
extern "C" {
    fn getrusage(who: i32, usage: *mut Rusage) -> i32;
}
fn cpu() -> (f64, f64) {
    let mut r: Rusage = unsafe { std::mem::zeroed() };
    unsafe { getrusage(0, &mut r) };
    let t = |tv: [i64; 2]| tv[0] as f64 + tv[1] as f64 / 1e6;
    (t(r.utime), t(r.stime))
}

struct Timer(Instant, (f64, f64), Vec<(String, f64, f64, f64)>);
impl Timer {
    fn new() -> Self {
        Timer(Instant::now(), cpu(), Vec::new())
    }
    fn tick(&mut self, what: &str) {
        let c = cpu();
        self.2.push((what.into(), self.0.elapsed().as_secs_f64(), c.0 - self.1 .0, c.1 - self.1 .1));
        self.0 = Instant::now();
        self.1 = c;
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (chunk, avsc, spec, runs) = (&a[1], &a[2], &a[3], a[4].parse::<usize>().unwrap());
    let only = a.get(5).cloned();
    let known: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(avsc).unwrap()).unwrap();
    let spec: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(spec).unwrap()).unwrap();
    let target = data_type_from_spec(&spec).unwrap();
    let invariants: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let prune: HashSet<String> = ["mainsnak__string", "value__string", "value", "error"].iter().map(|s| s.to_string()).collect();
    let cfg = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root: Some("claims".into()),
    };
    let mut pruned_schema = known.clone();
    prune_schema(&mut pruned_schema, &prune);
    let fields = avro_record_fields(&pruned_schema, false).unwrap();
    let plan = NormalisePlan::new(&known, &cfg, &prune);
    let arrow_plan = plan.to_arrow_strict(&fields).expect("compiles strictly");
    let lookup_path = std::env::temp_dir().join(format!("bench_lookup_{}.parquet", std::process::id()));
    let reps = |stage: &str| if only.as_deref() == Some(stage) { runs } else if only.is_some() { 1 } else { runs };
    let mut totals: HashMap<String, Vec<(f64, f64, f64)>> = HashMap::new();
    let runs_all = if only.is_some() { 1 } else { runs };
    for run in 0..runs_all {
        let mut t = Timer::new();
        let mut column = None;
        for _ in 0..reps("read") {
            column = Some(read_string_column_arrays(chunk, "claims").unwrap());
        }
        let keep = read_columns(chunk, &["id".to_string()]).unwrap();
        t.tick("read");
        let column = column.unwrap();
        let rows = column.rows();
        let mut extracted = None;
        for _ in 0..reps("extract") {
            let e = extract_invariants_rows(&rows, &invariants, Scan::Keyed).unwrap();
            if run == 0 {
                let (mut b, mut bb, mut o, mut ob, mut orig) = (0, 0, 0, 0, 0);
                for (r, s) in e.rows.iter().zip(&rows) {
                    match r {
                        Some(std::borrow::Cow::Borrowed(x)) => { b += 1; bb += x.len(); }
                        Some(std::borrow::Cow::Owned(x)) => { o += 1; ob += x.len(); orig += s.map_or(0, |s| s.len()); }
                        None => {}
                    }
                }
                eprintln!("rows borrowed {b} ({:.1} MB), rewritten {o} ({:.1} MB, from {:.1} MB), lookup {}", bb as f64 / 1e6, ob as f64 / 1e6, orig as f64 / 1e6, e.lookup.len());
            }
            t.tick("extract");
            extracted = Some(Extracted::from(e));
        }
        t.tick("extract (owned)");
        let extracted = extracted.unwrap();
        drop(column);
        let slim: Vec<Option<&str>> = extracted.rows.iter().map(Option::as_deref).collect();
        let batch_rows = slim.len().div_ceil(rayon::current_num_threads() * 4).max(1);
        let mut decoded = None;
        for _ in 0..reps("decode") {
            drop(decoded.take());
            let d: Option<Vec<_>> = slim
                .par_chunks(batch_rows)
                .enumerate()
                .map(|(b, chunk)| arrow_plan.normalise(chunk, b * batch_rows))
                .collect();
            decoded = Some(d.expect("every batch decodes strictly"));
        }
        t.tick("decode");
        for _ in 0..reps("lookup") {
            write_lookup_table(lookup_path.to_str().unwrap(), &extracted.lookup).unwrap();
        }
        t.tick("lookup");
        let (arrays, _pruned): (Vec<_>, Vec<_>) = decoded.unwrap().into_iter().unzip();
        let mut conformed = None;
        for _ in 0..reps("conform") {
            let c: Vec<arrow::array::StructArray> = arrays
                .par_iter()
                .map(|a| {
                    let a: ArrayRef = std::sync::Arc::new(a.clone());
                    conform(&a, &target).map(|c| c.as_any().downcast_ref::<arrow::array::StructArray>().unwrap().clone())
                })
                .collect::<Result<_, _>>()
                .unwrap();
            conformed = Some(c);
        }
        drop(arrays);
        t.tick("conform");
        let arrow::datatypes::DataType::Struct(tfields) = &target else { unreachable!() };
        let (_schema, batches) = struct_batches("claims", conformed.unwrap(), tfields, &keep, None, true).unwrap();
        t.tick("batches");
        drop(batches);
        drop(extracted);
        t.tick("drop");
        if run > 0 || runs_all == 1 {
            for (k, w, u, s) in t.2 {
                totals.entry(k).or_default().push((w, u, s));
            }
        }
    }
    let _ = std::fs::remove_file(&lookup_path);
    let mut sum = (0.0, 0.0, 0.0);
    for k in ["read", "extract", "extract (owned)", "decode", "lookup", "conform", "batches", "drop"] {
        let v = &totals[k];
        let n = v.len() as f64 * reps(k) as f64;
        let m = |f: fn(&(f64, f64, f64)) -> f64| v.iter().map(f).sum::<f64>() / n;
        let (w, u, s) = (m(|x| x.0), m(|x| x.1), m(|x| x.2));
        sum = (sum.0 + w, sum.1 + u, sum.2 + s);
        println!("{k:8} wall {w:.3} user {u:.3} sys {s:.3}");
    }
    println!("total    wall {:.3} user {:.3} sys {:.3}", sum.0, sum.1, sum.2);
}
