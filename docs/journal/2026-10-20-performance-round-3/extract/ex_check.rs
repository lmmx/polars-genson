//! MEASUREMENT ONLY (research, not for the repository): extraction on a Parquet column of
//! claims, by the scan, against the `Value` path that is its reference; and a hash of the
//! scan's exact output (slim rows' text and lookup entries) to compare across builds.
//! Usage: ex_check <chunk.parquet> [--reference]
use genson_core::extract::{extract_invariants_by_value, extract_invariants_from};
use std::hash::{Hash, Hasher};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let rows = column.rows();
    let spec: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
        .iter()
        .map(|(f, k)| (f.to_string(), k.to_string()))
        .collect();
    let t = std::time::Instant::now();
    let fast = extract_invariants_from(&rows, &spec).unwrap();
    let took = t.elapsed().as_secs_f64();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    fast.rows.hash(&mut h);
    for e in &fast.lookup {
        (&e.field, &e.key, e.value.to_string()).hash(&mut h);
    }
    print!("rows {} lookup {} hash {:016x} scan {:.3}s", fast.rows.len(), fast.lookup.len(), h.finish(), took);
    if args.get(2).map(String::as_str) == Some("--reference") {
        let slow = extract_invariants_by_value(&rows, &spec).unwrap();
        let parsed = |t: &Option<String>| t.as_deref().map(|t| serde_json::from_str::<serde_json::Value>(t).ok());
        let same_rows = fast.rows.iter().zip(&slow.rows).filter(|(f, s)| parsed(f) == parsed(s)).count();
        print!(" | reference: lookup equal {} rows equal {}/{}", fast.lookup == slow.lookup, same_rows, slow.rows.len());
    }
    println!();
}
