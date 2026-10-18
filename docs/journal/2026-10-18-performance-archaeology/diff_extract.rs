//! MEASUREMENT ONLY: `extract_invariants_from` (rows scanned as text) against
//! `extract_invariants_by_value` (rows read as `Value`s) on a Parquet string column, with
//! wikidata-pq's spec. Checks that the slim rows parse to the same values with keys in the
//! same order, that the lookup tables are equal, and that the schemas inferred from the slim
//! rows under wikidata-pq's claims options are equal; times each extraction (all threads).
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `diff_extract chunk.parquet claims`
use genson_core::extract::{extract_invariants_by_value, extract_invariants_from};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
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
    let column = genson_core::parquet::read_string_column_arrays(&args[1], &args[2]).unwrap();
    let rows = column.rows();
    let spec: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
        .iter()
        .map(|(f, k)| (f.to_string(), k.to_string()))
        .collect();
    let mut fast_t = Vec::new();
    let mut slow_t = Vec::new();
    let (mut fast, mut slow) = (None, None);
    for _ in 0..3 {
        let t = Instant::now();
        fast = Some(extract_invariants_from(&rows, &spec).unwrap());
        fast_t.push(t.elapsed().as_secs_f64());
        let t = Instant::now();
        slow = Some(extract_invariants_by_value(&rows, &spec).unwrap());
        slow_t.push(t.elapsed().as_secs_f64());
    }
    let (fast, slow) = (fast.unwrap(), slow.unwrap());
    assert_eq!(fast.lookup, slow.lookup, "lookup tables differ");
    let mut same_text = 0;
    for (i, (f, s)) in fast.rows.iter().zip(&slow.rows).enumerate() {
        match (f, s) {
            (Some(f), Some(s)) => {
                same_text += usize::from(f == s);
                let a: serde_json::Value = serde_json::from_str(f).unwrap();
                let b: serde_json::Value = serde_json::from_str(s).unwrap();
                assert_eq!(a.to_string(), b.to_string(), "row {i} differs");
            }
            (f, s) => assert_eq!(f, s, "row {i}"),
        }
    }
    let infer = |rows: &[Option<String>]| {
        let present: Vec<&str> = rows.iter().flatten().map(String::as_str).collect();
        infer_json_schema_from_strings(&present, claims_config()).unwrap().schema
    };
    assert_eq!(infer(&fast.rows), infer(&slow.rows), "inferred schemas differ");
    let bytes: usize = rows.iter().flatten().map(|r| r.len()).sum();
    println!(
        "{} rows, {} MB: identical lookup ({} entries), slim rows equal ({} of them byte-identical), equal schemas; scan {:.3}/{:.3}/{:.3}s, by value {:.3}/{:.3}/{:.3}s",
        rows.len(), bytes / 1_000_000, fast.lookup.len(), same_text,
        fast_t[0], fast_t[1], fast_t[2], slow_t[0], slow_t[1], slow_t[2]
    );
}
