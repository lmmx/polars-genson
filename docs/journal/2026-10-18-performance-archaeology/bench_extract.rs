//! MEASUREMENT ONLY: `extract_invariants` on an NDJSON corpus, rows as read from the
//! column (normalise_from_parquet extracts before `wrap_root`), with wikidata-pq's spec
//! (`labels`→`id`, `property-labels`→`property`, `unit-labels`→`unit`). Prints wall time,
//! the lookup size and the slim rows' bytes. RAYON_NUM_THREADS sets the threads.
//!
//! Copy to genson-core/examples/ and run: `bench_extract rows.ndjson`
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let rows: Vec<Option<String>> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| Some(l.to_string()))
        .collect();
    let spec: Vec<(String, String)> = [("labels", "id"), ("property-labels", "property"), ("unit-labels", "unit")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let input: usize = rows.iter().flatten().map(String::len).sum();
    let t = Instant::now();
    let out = genson_core::extract::extract_invariants(rows, &spec).unwrap();
    let dt = t.elapsed();
    let slim: usize = out.rows.iter().flatten().map(String::len).sum();
    println!(
        "extract_invariants {:.3}s: {} MB in, {} MB slim, {} lookup entries",
        dt.as_secs_f64(),
        input / 1_000_000,
        slim / 1_000_000,
        out.lookup.len()
    );
}
