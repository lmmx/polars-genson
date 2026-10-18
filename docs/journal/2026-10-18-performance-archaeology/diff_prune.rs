//! MEASUREMENT ONLY: a differential check of `normalise_values_pruned` between builds.
//! Generates random rows (records in records and arrays, keys encoding their path, as the
//! property test in genson-core/src/tests/normalise.rs does), infers each batch's Avro
//! schema, normalises with random prune names and with none, and prints a hash of every
//! output row and pruned value (row, path, value), with counts. A case whose normalisation
//! panics (a schema with two records of one name, as Avro conversion can give: not
//! handled by `normalise` on either side) counts as a panic, in the hash too. Two builds
//! that print the same line normalise, prune and panic on these cases alike.
//!
//! Copy to genson-core/examples/ and run with `--features avro`: `diff_prune [cases]`
use genson_core::normalise::{normalise_values_pruned, NormaliseConfig};
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn document(rng: &mut Rng, key: &str, depth: u32) -> Value {
    let pick = if depth == 0 { 3 + rng.below(4) } else { rng.below(7) };
    match pick {
        0 | 1 => record(rng, key, depth),
        2 => {
            let scalar = [json!(1), json!("s"), json!(true)][rng.below(3) as usize].clone();
            let records = rng.below(2) == 0;
            let items = (0..rng.below(4)).map(|_| {
                if records {
                    record(rng, key, depth)
                } else {
                    scalar.clone()
                }
            });
            Value::Array(items.collect())
        }
        3 => Value::Null,
        4 => json!(rng.below(10)),
        5 => json!("s"),
        _ => json!(true),
    }
}

fn record(rng: &mut Rng, key: &str, depth: u32) -> Value {
    let n = 1 + rng.below(3);
    let fields = (0..n).map(|_| {
        let key = format!("{key}{}", ["a", "b", "c"][rng.below(3) as usize]);
        let value = document(rng, &key, depth - 1);
        (key, value)
    });
    Value::Object(fields.collect())
}

fn keys() -> Vec<String> {
    let mut keys = vec![String::new()];
    let mut all = Vec::new();
    for _ in 0..4 {
        keys = keys
            .iter()
            .flat_map(|k| ["a", "b", "c"].map(|l| format!("{k}{l}")))
            .collect();
        all.extend(keys.iter().cloned());
    }
    all
}

fn main() {
    let cases: usize = std::env::args().nth(1).map_or(2000, |n| n.parse().unwrap());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let cfg = NormaliseConfig::default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let (mut ran, mut pruned_values, mut pruned_cases, mut panics) = (0, 0, 0, 0);
    std::panic::set_hook(Box::new(|_| {}));
    for _ in 0..cases {
        let rows: Vec<Value> = (0..1 + rng.below(6))
            .map(|_| json!({"r": document(&mut rng, "", 4)}))
            .collect();
        let prune: HashSet<String> = keys().into_iter().filter(|_| rng.below(8) == 0).collect();
        let strings: Vec<String> = rows.iter().map(Value::to_string).collect();
        let config = SchemaInferenceConfig {
            avro: true,
            ..Default::default()
        };
        let Ok(inferred) = infer_json_schema_from_strings(&strings, config) else {
            continue;
        };
        ran += 1;
        for names in [&prune, &HashSet::new()] {
            let result = std::panic::catch_unwind(|| {
                normalise_values_pruned(rows.clone(), &inferred.schema, &cfg, names)
            });
            let Ok((out, pruned)) = result else {
                panics += 1;
                "panic".hash(&mut hasher);
                continue;
            };
            serde_json::to_string(&out).unwrap().hash(&mut hasher);
            serde_json::to_string(&pruned).unwrap().hash(&mut hasher);
            pruned_values += pruned.len();
            pruned_cases += usize::from(!pruned.is_empty());
        }
    }
    println!(
        "cases {ran} of {cases}, {pruned_values} values pruned in {pruned_cases}, {panics} panics, hash {:x}",
        hasher.finish()
    );
}
