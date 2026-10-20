//! MEASUREMENT ONLY (research/profile): reading a string column as normalise_from_parquet
//! does (`read_string_column_arrays`, parts = 4 x threads) against fewer parts, and the
//! keep column (`read_columns`). Usage: prof3_read <chunk.parquet> <column> [runs]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args.get(3).map_or(5, |s| s.parse().unwrap());
    for run in 0..runs {
        let mut out = Vec::new();
        for parts in [1usize, 4, 16] {
            let t = std::time::Instant::now();
            let col = genson_core::parquet::read_string_column_parts(&args[1], &args[2], parts).unwrap();
            let n = col.rows().len();
            out.push(format!("\"parts{parts}\":{:.4}", t.elapsed().as_secs_f64()));
            if parts == 16 {
                out.push(format!("\"rows\":{n}"));
            }
        }
        let t = std::time::Instant::now();
        let keep = genson_core::parquet::read_columns(&args[1], &["id".to_string()]).unwrap();
        out.push(format!("\"keep_id\":{:.4},\"keep_cols\":{}", t.elapsed().as_secs_f64(), keep.len()));
        println!("{{\"run\":{run},{}}}", out.join(","));
    }
}
