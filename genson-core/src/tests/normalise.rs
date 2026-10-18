use super::*;
use serde_json::json;

#[test]
fn test_normalise_record() {
    let schema = json!({
        "type": "record",
        "name": "doc",
        "fields": [
            {"name": "id", "type": "string"},
            {"name": "labels", "type": {"type": "map", "values": "string"}},
        ]
    });

    let cfg = NormaliseConfig::default();
    let input = json!({"id": 42}); // id is number, labels missing
    let normalised = normalise_value(input, &schema, &cfg, None);

    assert_eq!(normalised, json!({"id": "42", "labels": Value::Null}));
}

#[test]
fn test_normalise_array_union() {
    let schema = json!(["null", {"type": "array", "items": "string"}]);
    let cfg = NormaliseConfig::default();

    let input = json!("hello"); // scalar string
    let normalised = normalise_value(input, &schema, &cfg, None);

    assert_eq!(normalised, json!(["hello"]));
}

#[test]
fn test_empty_map_to_null() {
    let schema = json!({"type": "map", "values": "string"});
    let cfg = NormaliseConfig::default();

    let input = json!({});
    let normalised = normalise_value(input, &schema, &cfg, None);

    assert_eq!(normalised, Value::Null);
}

#[test]
fn test_empty_map_preserved_if_flag_off() {
    let schema = json!({"type": "map", "values": "string"});
    let cfg = NormaliseConfig {
        empty_as_null: false,
        ..NormaliseConfig::default()
    };

    let input = json!({});
    let normalised = normalise_value(input, &schema, &cfg, None);

    assert_eq!(normalised, json!({}));
}

#[test]
fn test_string_coercion_toggle() {
    let schema = json!({
        "type": "record",
        "name": "doc",
        "fields": [
            {"name": "int_field", "type": "int"},
            {"name": "bool_field", "type": "boolean"},
        ]
    });

    let input = json!({
        "int_field": "42",
        "bool_field": "true"
    });

    // Default: coerce_string = false
    let cfg_no_coerce = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        ..NormaliseConfig::default()
    };
    let norm_no_coerce = normalise_value(input.clone(), &schema, &cfg_no_coerce, None);
    assert_eq!(
        norm_no_coerce,
        json!({
            "int_field": Value::Null,     // stays null because string not coerced
            "bool_field": Value::Null     // same here
        })
    );

    // With coerce_string = true
    let cfg_coerce = NormaliseConfig {
        empty_as_null: true,
        coerce_string: true,
        ..NormaliseConfig::default()
    };
    let norm_coerce = normalise_value(input, &schema, &cfg_coerce, None);
    assert_eq!(
        norm_coerce,
        json!({
            "int_field": 42,
            "bool_field": true
        })
    );
}

#[test]
fn test_normalise_map_of_records() {
    // Schema: map<string, record{language:string, value:string}>
    let schema = json!({
        "type": "map",
        "values": {
            "type": "object",
            "properties": {
                "language": { "type": "string" },
                "value": { "type": "string" }
            },
            "required": ["language", "value"]
        }
    });

    // Input data
    let input = json!({
        "en": { "language": "en", "value": "Hello" },
        "fr": { "language": "fr", "value": "Bonjour" }
    });

    let cfg = NormaliseConfig::default();

    let normalised = normalise_value(input, &schema, &cfg, None);

    // Expect same shape back (since it's already valid against schema)
    let expected = json!({
        "en": { "language": "en", "value": "Hello" },
        "fr": { "language": "fr", "value": "Bonjour" }
    });

    assert_eq!(normalised, expected);
}

#[test]
fn test_normalise_map_of_records_with_null() {
    // Same schema as before
    let schema = json!({
        "type": "map",
        "values": {
            "type": "object",
            "properties": {
                "language": { "type": "string" },
                "value": { "type": "string" }
            },
            "required": ["language", "value"]
        }
    });

    // Input with a null value (should normalise to null for that entry)
    let input = json!({
        "en": { "language": "en", "value": "Hello" },
        "fr": null
    });

    let cfg = NormaliseConfig::default();

    let normalised = normalise_value(input, &schema, &cfg, None);

    let expected = json!({
        "en": { "language": "en", "value": "Hello" },
        "fr": null
    });

    assert_eq!(normalised, expected);
}

/// A small deterministic generator for the property test below.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

/// A random document nesting records in records and arrays. A record's keys extend its
/// own key by a letter (`a`, `ab`, `abc`), so rows share keys where they share a path but
/// no two paths do: genson's Avro schema names a record after its key, and two records of
/// one name but different fields are a case `normalise` does not handle and this test is
/// not about. An array's items are records, or else all of one scalar type.
fn random_document(rng: &mut Rng, key: &str, depth: u32) -> Value {
    let pick = if depth == 0 { 3 + rng.below(4) } else { rng.below(7) };
    match pick {
        0 | 1 => random_record(rng, key, depth),
        2 => {
            let scalar = [json!(1), json!("s"), json!(true)][rng.below(3) as usize].clone();
            let records = rng.below(2) == 0;
            let items = (0..rng.below(4)).map(|_| {
                if records {
                    random_record(rng, key, depth)
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

fn random_record(rng: &mut Rng, key: &str, depth: u32) -> Value {
    let n = 1 + rng.below(3);
    let fields = (0..n).map(|_| {
        let key = format!("{key}{}", ["a", "b", "c"][rng.below(3) as usize]);
        let value = random_document(rng, &key, depth - 1);
        (key, value)
    });
    Value::Object(fields.collect())
}

/// Every key `random_document` can write below the root, to prune by.
fn random_document_keys() -> Vec<String> {
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

/// The input value at `path` from a row's root. A value that is not an array where the
/// schema has one is normalised as an array of itself, so its index 0 is itself.
fn at<'a>(value: &'a Value, path: &[PathSegment]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, segment| match segment {
        PathSegment::Key(k) => v.get(k),
        PathSegment::Index(0) if !v.is_array() => Some(v),
        PathSegment::Index(i) => v.get(i),
    })
}

/// For random rows and prune names: every pruned value is the input as it was at its path
/// (taking a record's fields out of its input leaves the records that are pruned whole), and
/// names that no record holds prune nothing and change nothing.
#[cfg(feature = "avro")]
#[test]
fn test_pruned_values_are_the_input_at_their_paths() {
    use crate::{infer_json_schema_from_strings, SchemaInferenceConfig};
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let cfg = NormaliseConfig::default();
    let mut pruned_seen = 0;
    for _ in 0..500 {
        let rows: Vec<Value> = (0..1 + rng.below(6))
            .map(|_| json!({"r": random_document(&mut rng, "", 4)}))
            .collect();
        let strings: Vec<String> = rows.iter().map(Value::to_string).collect();
        let config = SchemaInferenceConfig {
            avro: true,
            ..Default::default()
        };
        let Ok(inferred) = infer_json_schema_from_strings(&strings, config) else {
            continue;
        };
        let prune: HashSet<String> = random_document_keys()
            .into_iter()
            .filter(|_| rng.below(8) == 0)
            .collect();

        let (out, pruned) = normalise_values_pruned(rows.clone(), &inferred.schema, &cfg, &prune);
        assert_eq!(out.len(), rows.len());
        for p in &pruned {
            assert_eq!(at(&rows[p.row], &p.path), Some(&p.value), "{strings:?} pruning {prune:?}");
        }
        pruned_seen += pruned.len();

        let absent: HashSet<String> = ["zz".to_string()].into();
        let (unpruned, none) = normalise_values_pruned(rows.clone(), &inferred.schema, &cfg, &absent);
        assert!(none.is_empty());
        assert_eq!(unpruned, normalise_values(rows, &inferred.schema, &cfg), "{strings:?}");
    }
    assert!(pruned_seen > 100, "the rows should prune something: {pruned_seen}");
}

