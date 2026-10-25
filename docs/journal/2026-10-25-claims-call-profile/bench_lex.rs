use genson_core::parquet::read_string_column_arrays;
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let col = read_string_column_arrays(&a[1], "claims").unwrap();
    let rows: Vec<&str> = col.rows().into_iter().flatten().collect();
    let bytes: usize = rows.iter().map(|r| r.len()).sum();
    let mut buf = Vec::new();
    for round in 0..4 {
        let t = Instant::now();
        let n = genson_core::bench_lex(&rows);
        let ours = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut m = 0;
        for r in &rows {
            buf.clear();
            buf.extend_from_slice(r.as_bytes());
            m += simd_json::to_tape(&mut buf).map(|t| t.0.len()).unwrap_or(0);
        }
        let simd = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut k = 0;
        for r in &rows {
            buf.clear();
            buf.extend_from_slice(r.as_bytes());
            k += buf.len();
        }
        let copy = t.elapsed().as_secs_f64();
        if round > 0 {
            println!("{:.1} MB: ours {ours:.3}s ({:.0} MB/s, {n} toks), simd-json to_tape {simd:.3}s ({:.0} MB/s, {m} nodes, copy {copy:.3}s) {k}", bytes as f64 / 1e6, bytes as f64 / 1e6 / ours, bytes as f64 / 1e6 / (simd - copy));
        }
    }
}
