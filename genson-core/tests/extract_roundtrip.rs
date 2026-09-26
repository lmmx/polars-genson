use genson_core::extract::{extract_lookup, LookupEntry};
use serde_json::Value;
use std::collections::HashMap;

fn spec() -> Vec<(String, String)> {
    [
        ("labels", "id"),
        ("property-labels", "property"),
        ("unit-labels", "unit"),
    ]
    .iter()
    .map(|(f, k)| (f.to_string(), k.to_string()))
    .collect()
}

/// `slim` equals `orig` except that each extracted field is absent from `slim` and its
/// subtree is the lookup entry for its key. Returns how many fields were checked.
fn check(orig: &Value, slim: &Value, lookup: &HashMap<(String, String), Value>) -> usize {
    match (orig, slim) {
        (Value::Object(o), Value::Object(s)) => {
            let mut checked = 0;
            for (k, v) in o {
                let keyed_by = spec().into_iter().find(|(f, _)| f == k).map(|(_, kf)| kf);
                let key = keyed_by.and_then(|kf| match o.get(&kf) {
                    Some(Value::String(x)) => Some(x.clone()),
                    Some(Value::Null) | None => None,
                    Some(other) => Some(other.to_string()),
                });
                match key {
                    Some(key) => {
                        assert!(!s.contains_key(k), "'{k}' left in slim row");
                        assert_eq!(&lookup[&(k.clone(), key)], v, "lookup value for '{k}'");
                        checked += 1;
                    }
                    None => checked += check(v, &s[k], lookup),
                }
            }
            assert_eq!(
                s.keys().collect::<Vec<_>>(),
                o.keys().filter(|k| s.contains_key(*k)).collect::<Vec<_>>(),
                "slim row has extra or reordered keys"
            );
            checked
        }
        (Value::Array(o), Value::Array(s)) => {
            assert_eq!(o.len(), s.len());
            o.iter().zip(s).map(|(a, b)| check(a, b, lookup)).sum()
        }
        _ => {
            assert_eq!(orig, slim);
            0
        }
    }
}

#[test]
fn test_extract_lookup_round_trip_on_claims_fixtures() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../genson-cli/tests/data/claims"
    );
    let mut total = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let rows: Vec<Option<String>> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Some(l.to_string()))
            .collect();
        let out = extract_lookup(rows.clone(), &spec()).unwrap();
        let lookup: HashMap<(String, String), Value> = out
            .lookup
            .iter()
            .map(|LookupEntry { field, key, value }| ((field.clone(), key.clone()), value.clone()))
            .collect();
        assert_eq!(lookup.len(), out.lookup.len(), "duplicate lookup entries");
        for (orig, slim) in rows.iter().zip(&out.rows) {
            let orig: Value = serde_json::from_str(orig.as_deref().unwrap()).unwrap();
            let slim: Value = serde_json::from_str(slim.as_deref().unwrap()).unwrap();
            total += check(&orig, &slim, &lookup);
        }
    }
    assert!(total > 0, "fixtures contained no extractable fields");
    eprintln!("checked {total} extracted fields");
}
