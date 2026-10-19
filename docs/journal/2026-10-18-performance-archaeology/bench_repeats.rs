//! MEASUREMENT ONLY: how much of the rows' schema trees repeats. Each row's schema (as its
//! builder writes it), and at every depth each node's schema: how many nodes the rows' trees
//! hold in all, and how many distinct schemas among them.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`:
//! `bench_repeats chunk.parquet`
use genson_core::genson_rs::{build_json_schema_into, get_builder, BuildConfig};
use serde_json::Value;
use std::collections::HashSet;

fn walk(v: &Value, depth: usize, seen: &mut Vec<HashSet<String>>, total: &mut Vec<usize>) {
    if seen.len() <= depth {
        seen.push(HashSet::new());
        total.push(0);
    }
    total[depth] += 1;
    seen[depth].insert(v.to_string());
    if let Value::Object(m) = v {
        for (k, child) in m {
            match (k.as_str(), child) {
                ("properties" | "patternProperties", Value::Object(props)) => {
                    for s in props.values() {
                        walk(s, depth + 1, seen, total);
                    }
                }
                ("items" | "additionalProperties", s @ Value::Object(_)) => walk(s, depth + 1, seen, total),
                ("anyOf", Value::Array(xs)) => xs.iter().for_each(|s| walk(s, depth + 1, seen, total)),
                _ => {}
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let rows: Vec<&str> = column.rows().into_iter().flatten().collect();
    let cfg = BuildConfig { delimiter: Some(b'\n'), ignore_outer_array: false };
    let (mut seen, mut total) = (Vec::new(), Vec::new());
    for r in &rows {
        let mut b = get_builder(None);
        let mut bytes = [&b"{\"claims\":"[..], r.as_bytes(), b"}"].concat();
        build_json_schema_into(&mut b, &mut bytes, &cfg);
        let s = b.to_schema();
        walk(&s["properties"]["claims"], 0, &mut seen, &mut total);
    }
    let all: usize = total.iter().sum();
    let distinct: usize = seen.iter().map(HashSet::len).sum();
    println!("{} rows: {all} schema nodes, {distinct} distinct by depth ({:.1}x repetition)", rows.len(), all as f64 / distinct as f64);
    for (d, (t, s)) in total.iter().zip(&seen).enumerate() {
        println!("  depth {d}: {t} nodes, {} distinct", s.len());
    }
}
