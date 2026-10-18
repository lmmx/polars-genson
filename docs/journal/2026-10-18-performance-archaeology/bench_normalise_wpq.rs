//! MEASUREMENT ONLY: the typed row loop of polars-genson-py's `normalise_from_parquet` on a
//! Parquet chunk's claims column, as wikidata-pq's `normalise_claims_direct` runs it: the
//! schema inferred once with its claims options, then per row `serde_json::from_str`,
//! `normalise_values_pruned` (key/value maps, `prune=QUARANTINE_FIELDS`) and, over all rows,
//! `values_to_struct_array` with the pruned schema's fields. One thread unless THREADS=1,
//! which runs the loop as `normalise_from_parquet` does (batches over the rayon pool).
//! Rows are normalised with a `NormalisePlan` compiled once (genson-core >= the plan), or
//! with REF=1 by `normalise_values_pruned_by_schema`, which reads the schema for every
//! value. Prints each stage's time per run and a hash of the normalised rows and pruned
//! values, so a change can be checked for identical output.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_normalise_wpq chunk.parquet [runs]`
use genson_core::normalise::{
    normalise_values_pruned_by_schema, prune_schema, MapEncoding, NormaliseConfig, NormalisePlan,
};
use genson_core::parquet::{avro_record_fields, values_to_struct_array};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use rayon::prelude::*;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::time::Instant;

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
    let rows = column.rows();
    let present: Vec<&str> = rows.iter().flatten().copied().collect();
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
    let threads = std::env::var("THREADS").is_ok();
    let reference = std::env::var("REF").is_ok();
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    for _ in 0..runs {
        let t = Instant::now();
        let parsed: Vec<serde_json::Value> = present
            .iter()
            .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::Null))
            .collect();
        let t_parse = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let normalise = |v: serde_json::Value| {
            if reference {
                let (mut rows, pruned) =
                    normalise_values_pruned_by_schema(vec![v], &schema, &cfg, &prune);
                (rows.pop().unwrap(), pruned)
            } else {
                plan.normalise(v)
            }
        };
        let (normed, pruned): (Vec<_>, Vec<_>) = if threads {
            parsed.into_par_iter().map(normalise).unzip()
        } else {
            parsed.into_iter().map(normalise).unzip()
        };
        let t_norm = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let batch = normed.len().div_ceil(rayon::current_num_threads() * 4).max(1);
        let arrays: Vec<_> = if threads {
            normed.par_chunks(batch).map(|c| values_to_struct_array(c, &fields).unwrap()).collect()
        } else {
            normed.chunks(batch).map(|c| values_to_struct_array(c, &fields).unwrap()).collect()
        };
        let t_decode = t.elapsed().as_secs_f64();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for row in &normed {
            row.to_string().hash(&mut h);
        }
        let mut n_pruned = 0;
        for p in pruned.iter().flatten() {
            n_pruned += 1;
            serde_json::to_string(&p.path).unwrap().hash(&mut h);
            p.value.to_string().hash(&mut h);
        }
        let len: usize = arrays.iter().map(arrow::array::Array::len).sum();
        println!(
            "{} rows: parse {:.3}s normalise {:.3}s decode {:.3}s; {} pruned, {} decoded, hash={:x}",
            present.len(),
            t_parse,
            t_norm,
            t_decode,
            n_pruned,
            len,
            h.finish()
        );
    }
}
