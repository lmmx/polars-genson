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


/// `normalise_values_pruned` (by its compiled plan) and the reference that reads the schema
/// for every value, on the same rows, schema, options and prune names: equal rows and
/// pruned values, or the same panic. Returns how many values were pruned.
fn assert_plan_matches(
    rows: &[Value],
    schema: &Value,
    cfg: &NormaliseConfig,
    prune: &HashSet<String>,
) -> usize {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let message = |e: Box<dyn std::any::Any + Send>| {
        e.downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
    };
    let plan = catch_unwind(AssertUnwindSafe(|| {
        normalise_values_pruned(rows.to_vec(), schema, cfg, prune)
    }))
    .map_err(message);
    let reference = catch_unwind(AssertUnwindSafe(|| {
        normalise_values_pruned_by_schema(rows.to_vec(), schema, cfg, prune)
    }))
    .map_err(message);
    assert_eq!(
        plan, reference,
        "rows {rows:?}\nschema {schema}\nprune {prune:?}"
    );
    plan.map_or(0, |(_, pruned)| pruned.len())
}

/// Every option combination `assert_plan_matches` is run with.
fn normalise_configs() -> Vec<NormaliseConfig> {
    let mut configs = Vec::new();
    for map_encoding in [
        MapEncoding::Mapping,
        MapEncoding::Entries,
        MapEncoding::KeyValueEntries,
    ] {
        for (empty_as_null, coerce_string) in [(true, false), (false, true)] {
            for wrap_root in [None, Some("w".to_string())] {
                configs.push(NormaliseConfig {
                    empty_as_null,
                    coerce_string,
                    map_encoding,
                    wrap_root,
                });
            }
        }
    }
    configs
}

/// For random rows and schemas inferred from them (records, arrays, maps, promoted scalars,
/// unions), with and without prune names, under every option: the compiled plan normalises
/// and prunes as reading the schema does, also for rows of another batch than the schema's.
#[cfg(feature = "avro")]
#[test]
fn test_plan_matches_the_schema_reading_path() {
    use crate::{infer_json_schema_from_strings, SchemaInferenceConfig};
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let configs = normalise_configs();
    let keys = random_document_keys();
    let mut previous: Vec<Value> = Vec::new();
    let mut pruned_seen = 0;
    for case in 0..2000 {
        let rows: Vec<Value> = (0..1 + rng.below(6))
            .map(|_| {
                let mut row = json!({"r": random_document(&mut rng, "", 4)});
                // Strings that coerce, floats beside integers, and an empty map
                match rng.below(5) {
                    0 => row["n"] = json!(["1", "x", "2.5", "true"][rng.below(4) as usize]),
                    1 => {
                        row["n"] = [json!(1.5), json!(2), json!(-3)][rng.below(3) as usize].clone()
                    }
                    2 => row["m"] = json!({}),
                    _ => {}
                }
                row
            })
            .collect();
        let strings: Vec<String> = rows.iter().map(Value::to_string).collect();
        let config = SchemaInferenceConfig {
            avro: true,
            map_threshold: [20, 2, 0][case % 3],
            unify_maps: case % 2 == 0,
            wrap_scalars: case % 4 != 3,
            ..Default::default()
        };
        let Ok(inferred) = infer_json_schema_from_strings(&strings, config) else {
            continue;
        };
        let prune: HashSet<String> = keys.iter().filter(|_| rng.below(8) == 0).cloned().collect();
        let cfg = &configs[case % configs.len()];
        for prune in [HashSet::new(), prune] {
            pruned_seen += assert_plan_matches(&rows, &inferred.schema, cfg, &prune);
            assert_plan_matches(&previous, &inferred.schema, cfg, &prune);
        }
        previous = rows;
    }
    assert!(
        pruned_seen > 100,
        "the rows should prune something: {pruned_seen}"
    );
}

/// Schemas inference does not write, for the kinds of node normalisation treats specially:
/// names and other types it passes values through, records without `fields` or with
/// fields lacking a name or type, unions of only `null` and empty ones (which panic on a
/// value as before), and promoted records.
#[test]
fn test_plan_matches_on_unusual_schemas() {
    let schemas = [
        json!("null"),
        json!("Named"),
        json!({"type": "string"}),
        json!({"type": "fixed", "size": 2}),
        json!(7),
        json!({"type": "record"}),
        json!({"type": "record", "fields": {"a": "int"}}),
        json!({"type": "record", "fields": [
            {"name": "a"},
            {"type": "int"},
            "x",
            {"name": 1, "type": "int"},
            {"name": "b", "type": "long"},
        ]}),
        json!(["null"]),
        json!([]),
        json!(["string", "int"]),
        json!(["int", "null", "string"]),
        json!({"type": "array"}),
        json!({"type": "map"}),
        json!({"type": "array", "items": {"type": "map", "values": ["null", "double"]}}),
        json!({"type": "record", "name": "p", "fields": [
            {"name": "v__string", "type": ["null", "string"]},
            {"name": "v__int", "type": ["null", "long"]},
            {"name": "v__float", "type": ["null", "double"]},
            {"name": "v__boolean", "type": ["null", "boolean"]},
        ]}),
        json!({"type": "record", "name": "r", "fields": [
            {"name": "a", "type": {"type": "map", "values": {
                "type": "record",
                "name": "q",
                "fields": [
                    {"name": "x", "type": ["null", "string"]},
                    {"name": "y", "type": {"type": "array", "items": "boolean"}},
                ],
            }}},
            {"name": "b", "type": ["null", {"type": "array", "items": "int"}]},
        ]}),
    ];
    let values = [
        json!(null),
        json!(1),
        json!(-2),
        json!(1.5),
        json!("s"),
        json!("12"),
        json!("true"),
        json!(true),
        json!([]),
        json!([1, "a", null]),
        json!({}),
        json!({"a": 1, "b": [2], "v__string": "s"}),
        json!({"a": {"k": {"x": 1, "y": true}, "e": {}}, "b": 3}),
        json!({"a": "scalar", "b": null}),
    ];
    let prunes: [HashSet<String>; 3] = [
        HashSet::new(),
        ["x".to_string(), "v__int".to_string()].into(),
        ["b".to_string(), "a".to_string()].into(),
    ];
    for schema in &schemas {
        for cfg in &normalise_configs() {
            for prune in &prunes {
                // One value at a time, so that a value that panics leaves the others tested
                for value in &values {
                    assert_plan_matches(std::slice::from_ref(value), schema, cfg, prune);
                }
                assert_plan_matches(&values, schema, cfg, prune);
            }
        }
    }
}

/// Random JSON text for the typed decoder: objects with keys from a small set (some
/// promoted, some repeated or escaped), arrays, numbers in every form `serde_json` reads
/// differently, escaped strings, and whitespace of every kind between tokens.
#[cfg(feature = "parquet")]
fn random_text(rng: &mut Rng, depth: u32, out: &mut String) {
    let ws = |rng: &mut Rng, out: &mut String| {
        if rng.below(4) == 0 {
            out.push_str([" ", "\n", "\t", "\r", "  "][rng.below(5) as usize]);
        }
    };
    let keys = [
        "a",
        "b",
        "c",
        "id",
        "v__string",
        "v__int",
        "v__float",
        "v__boolean",
        "v",
    ];
    ws(rng, out);
    // Now and then something `serde_json` rejects, making the row invalid
    if rng.below(60) == 0 {
        out.push_str(
            [
                "0123",
                "-012",
                "00",
                "1.",
                ".5",
                "+1",
                "1e",
                "--1",
                "nul",
                "tru",
                "\"\\x\"",
                "\"\\ud800\"",
                "\"\\udc00x\"",
                "\"a\tb\"",
                "[1,]",
                "{\"a\":1,}",
                "{\"a\" 1}",
                "1e999",
            ][rng.below(18) as usize],
        );
        return;
    }
    let pick = if depth == 0 {
        3 + rng.below(6)
    } else {
        rng.below(9)
    };
    match pick {
        0 | 1 => {
            out.push('{');
            let n = rng.below(5);
            for i in 0..n {
                if i > 0 {
                    out.push(',');
                }
                ws(rng, out);
                let key = keys[rng.below(keys.len() as u64) as usize];
                match rng.below(12) {
                    0 => out.push_str(&format!("\"\\u{:04x}{}\"", key.as_bytes()[0], &key[1..])),
                    _ => out.push_str(&format!("\"{key}\"")),
                }
                ws(rng, out);
                out.push(':');
                random_text(rng, depth - 1, out);
            }
            ws(rng, out);
            out.push('}');
        }
        2 => {
            out.push('[');
            let n = rng.below(4);
            for i in 0..n {
                if i > 0 {
                    out.push(',');
                }
                random_text(rng, depth - 1, out);
            }
            ws(rng, out);
            out.push(']');
        }
        3 => out.push_str("null"),
        4 => out.push_str(["true", "false"][rng.below(2) as usize]),
        5 | 6 => out.push_str(
            [
                "0",
                "-0",
                "12",
                "-12",
                "1.5",
                "-0.0",
                "1e2",
                "-1.25E-3",
                "2.5e+3",
                "123456789012345678901234",
                "9223372036854775807",
                "9223372036854775808",
                "-9223372036854775808",
                "-9223372036854775809",
                "18446744073709551615",
                "18446744073709551616",
                "999999999999999999",
                "1000000000000000000",
                "0.1",
                "100",
            ][rng.below(20) as usize],
        ),
        _ => out.push_str(
            [
                "\"s\"",
                "\"\"",
                "\"a\\nb\"",
                "\"\\u00e9t\\u00e9\"",
                "\"\\ud83d\\ude00\"",
                "\"q\\\"q\"",
                "\"12\"",
                "\"-3\"",
                "\"2.5\"",
                "\"true\"",
                "\"0\"",
                "\"nan\"",
                "\"inf\"",
                "\"1e999\"",
                "\"caf\u{e9}\"",
            ][rng.below(15) as usize],
        ),
    }
    ws(rng, out);
}

/// The typed rows the `Value` path gives: each row parsed (`null` if invalid), normalised
/// and decoded with `values_to_struct_array`; `Err` if any of it panics or fails.
#[cfg(feature = "parquet")]
fn typed_by_value(
    rows: &[Option<String>],
    plan: &NormalisePlan<'_>,
    fields: &arrow::datatypes::Fields,
) -> Result<(arrow::array::StructArray, Vec<PrunedValue>), String> {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    catch_unwind(AssertUnwindSafe(|| {
        let mut values = Vec::new();
        let mut pruned = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let Some(row) = row else {
                values.push(Value::Null);
                continue;
            };
            let v = serde_json::from_str(row).unwrap_or(Value::Null);
            let (normalised, mut row_pruned) = plan.normalise(v);
            row_pruned.iter_mut().for_each(|p| p.row = i);
            pruned.extend(row_pruned);
            values.push(normalised);
        }
        crate::parquet::values_to_struct_array(&values, fields).map(|a| (a, pruned))
    }))
    .unwrap_or_else(|_| Err("panicked".to_string()))
}

/// For random JSON text (valid, invalid and null rows), schemas inferred from the valid
/// rows (wrapped, with a field beside them or not, or as they are; with all of them or all
/// but the last) and every option and prune set: the typed decoder gives the arrays and
/// pruned values the `Value` path gives, or declines (`None`) where that path fails or
/// might differ, and declines rarely.
#[cfg(feature = "parquet")]
#[test]
fn test_typed_decode_matches_the_value_path() {
    use crate::{infer_json_schema_from_strings, SchemaInferenceConfig};
    let mut rng = Rng(0xd1b5_4a32_d192_ed03);
    let configs: Vec<NormaliseConfig> = normalise_configs()
        .into_iter()
        .filter(|c| c.map_encoding != MapEncoding::Entries)
        .collect();
    let prunes: [HashSet<String>; 4] = [
        HashSet::new(),
        ["v__string".to_string()].into(),
        ["a".to_string(), "v__int".to_string()].into(),
        ["b".to_string()].into(),
    ];
    let (mut decoded, mut declined, mut failing, mut pruned_seen) = (0, 0, 0, 0);
    let mut decoded_unwrapped = 0;
    for case in 0..2500 {
        // Mostly with each row wrapped as `w`, else the rows as they are: then mostly objects,
        // for a record at the root
        let wrap = case % 5 != 4;
        let mut rows: Vec<Option<String>> = Vec::new();
        for _ in 0..1 + rng.below(6) {
            let mut text = String::new();
            if !wrap && rng.below(8) != 0 {
                text.push_str("{\"a\":");
                random_text(&mut rng, 3, &mut text);
                text.push_str(",\"v\":");
                random_text(&mut rng, 3, &mut text);
                text.push('}');
            } else {
                random_text(&mut rng, 4, &mut text);
            }
            let text = match rng.below(14) {
                0 => text[..text.len() / 2].to_string(),
                1 => format!("{text} x"),
                2 => format!("{text}\n{text}"),
                _ => text,
            };
            rows.push(if rng.below(15) == 0 { None } else { Some(text) });
        }
        // Now and then the schema is inferred without the last row, which then need not fit
        // it, or with a field beside the wrapped row
        let inferred_from = match case % 7 {
            3 => &rows[..rows.len() - 1],
            _ => &rows[..],
        };
        let beside = case % 6 == 1;
        let strings: Vec<String> = inferred_from
            .iter()
            .flatten()
            .filter_map(|r| serde_json::from_str::<Value>(r).ok())
            .map(|v| match (wrap, beside) {
                (true, true) => json!({"w": v, "a": "x"}),
                (true, false) => json!({"w": v}),
                (false, _) => v,
            })
            .map(|v| v.to_string())
            .collect();
        if strings.is_empty() {
            continue;
        }
        let config = SchemaInferenceConfig {
            avro: true,
            map_threshold: [20, 2, 0][case % 3],
            unify_maps: case % 2 == 0,
            wrap_scalars: case % 4 != 3,
            ..Default::default()
        };
        let Ok(inferred) = infer_json_schema_from_strings(&strings, config) else {
            continue;
        };
        let cfg = &configs[case % configs.len()];
        let cfg = NormaliseConfig {
            wrap_root: wrap.then(|| "w".to_string()),
            ..cfg.clone()
        };
        if inferred.schema.get("type") != Some(&json!("record")) {
            continue;
        }
        let prune = &prunes[case % prunes.len()];
        let mut out_schema = inferred.schema.clone();
        prune_schema(&mut out_schema, prune);
        let native = cfg.map_encoding == MapEncoding::Mapping;
        let Ok(fields) = crate::parquet::avro_record_fields(&out_schema, native) else {
            continue;
        };
        let plan = NormalisePlan::new(&inferred.schema, &cfg, prune);
        let reference = typed_by_value(&rows, &plan, &fields);
        let borrowed: Vec<Option<&str>> = rows.iter().map(Option::as_deref).collect();
        let typed = plan
            .to_arrow(&fields)
            .and_then(|p| p.normalise(&borrowed, 0));
        match (typed, reference) {
            (Some(typed), Ok(reference)) => {
                assert_eq!(
                    typed.0, reference.0,
                    "rows {rows:?}\nschema {}",
                    inferred.schema
                );
                assert_eq!(
                    typed.1, reference.1,
                    "rows {rows:?}\nschema {}",
                    inferred.schema
                );
                pruned_seen += typed.1.len();
                decoded += 1;
                decoded_unwrapped += usize::from(!wrap);
            }
            (Some(typed), Err(e)) => {
                panic!(
                    "decoded {typed:?} where the Value path fails ({e}): rows {rows:?}\nschema {}",
                    inferred.schema
                )
            }
            (None, Ok(_)) => declined += 1,
            (None, Err(_)) => failing += 1,
        }
    }
    eprintln!("typed decode: decoded {decoded} ({decoded_unwrapped} unwrapped), declined {declined}, failing {failing}, pruned {pruned_seen}");
    assert!(
        decoded > 500,
        "decoded {decoded}, declined {declined}, failing {failing}"
    );
    assert!(
        declined * 20 < decoded,
        "decoded {decoded}, declined {declined}"
    );
    assert!(pruned_seen > 20, "pruned {pruned_seen}");
    assert!(
        decoded_unwrapped > 50,
        "decoded {decoded_unwrapped} unwrapped"
    );
}

/// Rows nested 127 deep are read, and 128 deep are invalid (null), as `serde_json` reads
/// them: in a string field, the one array written out and the other row null.
#[cfg(feature = "parquet")]
#[test]
fn test_typed_decode_depth_limit() {
    let schema = json!({
        "type": "record",
        "name": "document",
        "fields": [{"name": "w", "type": ["null", "string"]}]
    });
    let cfg = NormaliseConfig {
        wrap_root: Some("w".to_string()),
        ..Default::default()
    };
    let prune = HashSet::new();
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    let fields = crate::parquet::avro_record_fields(&schema, false).unwrap();
    let nested = |depth: usize| Some(format!("{}1{}", "[".repeat(depth), "]".repeat(depth)));
    let rows = vec![nested(127), nested(128)];
    let borrowed: Vec<Option<&str>> = rows.iter().map(Option::as_deref).collect();
    let typed = plan
        .to_arrow(&fields)
        .and_then(|p| p.normalise(&borrowed, 0))
        .unwrap();
    assert_eq!(typed, typed_by_value(&rows, &plan, &fields).unwrap());
    let w = typed
        .0
        .column(0)
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .unwrap();
    assert_eq!(w.value(0), nested(127).unwrap());
    assert!(arrow::array::Array::is_null(w, 1));
}

/// A record pruned by a field after one that could panic: a field of an empty union panics
/// on any value, null included, before the `Value` path reaches the pruning, so the typed
/// decoder declines. A field of `null` alone no longer panics (#238): whatever it holds is
/// null, and both prune the record.
#[cfg(feature = "parquet")]
#[test]
fn test_typed_decode_declines_a_prune_after_a_panic() {
    let schema = |a_type: Value| {
        json!({
            "type": "record",
            "name": "document",
            "fields": [
                {"name": "a", "type": a_type},
                {"name": "b", "type": ["null", {
                    "type": "record",
                    "name": "b",
                    "fields": [
                        {"name": "x", "type": ["null", "long"]},
                        {"name": "y", "type": ["null", "string"]}
                    ]
                }]}
            ]
        })
    };
    let cfg = NormaliseConfig::default();
    let prune: HashSet<String> = ["x".to_string()].into();
    let row = |text: &str| vec![Some(text.to_string())];
    let (held, null) = (
        row(r#"{"a": 1, "b": {"x": 1}}"#),
        row(r#"{"a": null, "b": {"x": 1}}"#),
    );
    for (a_type, panics) in [(json!(["null"]), false), (json!([]), true)] {
        let schema = schema(a_type);
        let plan = NormalisePlan::new(&schema, &cfg, &prune);
        let mut out_schema = schema.clone();
        prune_schema(&mut out_schema, &prune);
        let fields = crate::parquet::avro_record_fields(&out_schema, false).unwrap();
        let decode = |rows: &[Option<String>]| {
            let borrowed: Vec<Option<&str>> = rows.iter().map(Option::as_deref).collect();
            plan.to_arrow(&fields)
                .and_then(|p| p.normalise(&borrowed, 0))
        };
        for rows in [&held, &null] {
            if panics {
                assert!(typed_by_value(rows, &plan, &fields).is_err());
                assert!(decode(rows).is_none());
            } else {
                let typed = decode(rows).unwrap();
                assert_eq!(typed.1.len(), 1);
                assert_eq!(typed, typed_by_value(rows, &plan, &fields).unwrap());
            }
        }
    }
}

/// A union of `null` alone holds no other value: a value there is null, on the plan and on
/// the reference path, where both panicked (#238).
#[test]
fn test_null_only_union_normalises_a_value_to_null() {
    let schema = json!({
        "type": "record",
        "name": "r",
        "fields": [
            {"name": "d", "type": ["null"]},
            {"name": "e", "type": ["null", "int"]},
        ]
    });
    let cfg = NormaliseConfig::default();
    let prune = std::collections::HashSet::new();
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    for (input, expected) in [
        (json!({"d": 9, "e": 1}), json!({"d": null, "e": 1})),
        (json!({"d": null, "e": null}), json!({"d": null, "e": null})),
        (json!({"d": {"x": [1]}}), json!({"d": null, "e": null})),
    ] {
        assert_eq!(plan.normalise(input.clone()).0, expected);
        assert_eq!(normalise_value(input, &schema, &cfg, None), expected);
    }
}

/// #238's rows: their Avro schema names two differently shaped records alike, so the second
/// takes the first's fields and its `d` becomes `["null"]`. Normalising them no longer panics.
#[cfg(feature = "avro")]
#[test]
fn test_rows_with_one_record_name_for_two_shapes_normalise() {
    use crate::SchemaInferenceConfig;
    let rows = [
        r#"{"r":[[true,true,{"d":null}],1,{"d":9}]}"#,
        r#"{"r":true}"#,
        r#"{"r":true}"#,
    ];
    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        avro: true,
        ..Default::default()
    };
    let schema = crate::infer_json_schema_from_strings(&rows, config)
        .unwrap()
        .schema;
    let values: Vec<Value> = rows.iter().map(|r| serde_json::from_str(r).unwrap()).collect();
    let cfg = NormaliseConfig::default();
    let normalised = normalise_values(values.clone(), &schema, &cfg);
    assert_eq!(normalised.len(), 3);
    let prune = std::collections::HashSet::new();
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    let planned: Vec<Value> = values.into_iter().map(|v| plan.normalise(v).0).collect();
    assert_eq!(planned, normalised);
}

/// A value in a position typed `null` (a record field, or a list's items, as the Avro
/// conversion types those that were only ever null or empty) normalises to null on both
/// paths, and the rows decode, by the typed decoder too: kept as it was, it could not.
#[cfg(feature = "parquet")]
#[test]
fn test_a_value_typed_null_normalises_to_null() {
    let schema = json!({
        "type": "record",
        "name": "document",
        "fields": [
            {"name": "a", "type": ["null", "long"]},
            {"name": "n", "type": "null"},
            {"name": "l", "type": ["null", {"type": "array", "items": "null"}]}
        ]
    });
    let cfg = NormaliseConfig::default();
    let rows = [r#"{"a": 1, "n": 5, "l": ["x", null, {"b": 2}]}"#, r#"{"n": {"c": true}}"#];
    let values: Vec<Value> = rows.iter().map(|r| serde_json::from_str(r).unwrap()).collect();
    let expected = vec![
        json!({"a": 1, "n": null, "l": [null, null, null]}),
        json!({"a": null, "n": null, "l": null}),
    ];
    assert_eq!(normalise_values(values, &schema, &cfg), expected);
    let plan = NormalisePlan::new(&schema, &cfg, &HashSet::new());
    let fields = crate::parquet::avro_record_fields(&schema, false).unwrap();
    let owned: Vec<Option<String>> = rows.iter().map(|r| Some(r.to_string())).collect();
    let (by_value, _) = typed_by_value(&owned, &plan, &fields).unwrap();
    let borrowed: Vec<Option<&str>> = rows.iter().copied().map(Some).collect();
    let (typed, _) = plan.to_arrow(&fields).and_then(|p| p.normalise(&borrowed, 0)).unwrap();
    assert_eq!(typed, by_value);
}

/// A field that is a scalar in some rows and an empty list in the rest: normalisation wraps
/// the scalar into the list, so the list's items take the scalar's type, and the rows
/// decode with their values (before, the items were `null` and decoding failed).
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_empty_list_beside_scalars_decodes_their_values() {
    use crate::SchemaInferenceConfig;
    use arrow::array::{Array, ListArray, StringArray};
    for (empty_as_null, wrap_scalars) in [(true, true), (false, true), (true, false), (false, false)] {
        for rows in [
            vec![r#"{"s": "x"}"#, r#"{"s": []}"#],
            vec![r#"{"s": ""}"#, r#"{"s": "x"}"#, r#"{"s": []}"#],
            vec![r#"{"s": "x"}"#, r#"{"s": null}"#, r#"{"s": []}"#],
        ] {
            let config = SchemaInferenceConfig {
                delimiter: Some(b'\n'),
                avro: true,
                wrap_scalars,
                ..Default::default()
            };
            let schema = crate::infer_json_schema_from_strings(&rows, config).unwrap().schema;
            // A union of the list and the scalar, or (with `wrap_scalars`) the list alone
            let s = &schema["fields"][0]["type"];
            let branches = s.as_array().map_or(vec![s], |b| b.iter().collect());
            assert!(branches.iter().any(|t| t["items"] == "string"), "{s}");
            let cfg = NormaliseConfig {
                empty_as_null,
                ..Default::default()
            };
            let fields = crate::parquet::avro_record_fields(&schema, false).unwrap();
            let values: Vec<Value> = rows.iter().map(|r| serde_json::from_str(r).unwrap()).collect();
            let normalised = normalise_values(values, &schema, &cfg);
            let array = crate::parquet::values_to_struct_array(&normalised, &fields).unwrap();
            let list = array.column(0).as_any().downcast_ref::<ListArray>().unwrap();
            let first = list.value(0);
            let first = first.as_any().downcast_ref::<StringArray>().unwrap();
            assert_eq!(first.value(0), if rows[0].contains("\"\"") { "" } else { "x" });
        }
    }
}

/// An empty list beside a record: with `wrap_scalars`, one list of the record (normalisation
/// wraps the record into the list); without, a union whose list keeps `null` items.
#[cfg(feature = "avro")]
#[test]
fn test_empty_list_beside_a_record() {
    use crate::SchemaInferenceConfig;
    let rows = [r#"{"s": {"a": 1}}"#, r#"{"s": []}"#];
    for wrap_scalars in [true, false] {
        let config = SchemaInferenceConfig {
            delimiter: Some(b'\n'),
            avro: true,
            wrap_scalars,
            ..Default::default()
        };
        let schema = crate::infer_json_schema_from_strings(&rows, config).unwrap().schema;
        let s = &schema["fields"][0]["type"];
        if wrap_scalars {
            assert_eq!(s["items"]["type"], "record", "{s}");
        } else {
            assert!(s.as_array().unwrap().iter().any(|t| t["items"] == "null"), "{s}");
        }
    }
}

/// The non-null scalars in a value, counted (`sign` 1 to add, -1 to take away); numbers as
/// floats, as an integer in a float field widens.
#[cfg(all(feature = "avro", feature = "parquet"))]
fn count_scalars(value: &Value, counts: &mut std::collections::HashMap<String, i64>, sign: i64) {
    let key = match value {
        Value::Null => return,
        Value::Array(items) => return items.iter().for_each(|v| count_scalars(v, counts, sign)),
        Value::Object(m) => return m.values().for_each(|v| count_scalars(v, counts, sign)),
        Value::Number(n) => format!("{}", n.as_f64().unwrap()),
        scalar => scalar.to_string(),
    };
    *counts.entry(key).or_default() += sign;
}

/// Rows normalised against the Avro schema inferred from them: the schema and the rows,
/// which decode to Arrow, as the typed decoder decodes them where it takes them.
#[cfg(all(feature = "avro", feature = "parquet"))]
fn normalised_against_inferred(rows: &[&str], wrap_scalars: bool) -> (Value, Vec<Value>) {
    use crate::SchemaInferenceConfig;
    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        avro: true,
        wrap_scalars,
        ..Default::default()
    };
    let schema = crate::infer_json_schema_from_strings(rows, config).unwrap().schema;
    let cfg = NormaliseConfig::default();
    let plan = NormalisePlan::new(&schema, &cfg, &HashSet::new());
    let fields = crate::parquet::avro_record_fields(&schema, false).unwrap();
    let owned: Vec<Option<String>> = rows.iter().map(|r| Some(r.to_string())).collect();
    let (by_value, _) = typed_by_value(&owned, &plan, &fields).unwrap();
    let borrowed: Vec<Option<&str>> = rows.iter().copied().map(Some).collect();
    if let Some((typed, _)) = plan.to_arrow(&fields).and_then(|p| p.normalise(&borrowed, 0)) {
        assert_eq!(typed, by_value, "{schema}");
    }
    let values = rows.iter().map(|r| plan.normalise(serde_json::from_str(r).unwrap()).0);
    let normalised = values.collect();
    (schema, normalised)
}

/// A union of an array with records or scalars (as when a key holds a list in some places
/// and a record or a scalar in others, #238) is one array whose items take the other
/// branches: normalisation wraps their values into the array, so every value is kept, for
/// either order of the branches and beside `null`, and the rows decode.
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_union_of_an_array_and_other_branches_keeps_their_values() {
    let cases: [&[&str]; 9] = [
        &[
            r#"{"r":[[true,true,{"d":null}],1,{"d":9}]}"#,
            r#"{"r":true}"#,
            r#"{"r":true}"#,
        ],
        &[r#"{"r":[[{"d":null}],{"d":9}]}"#],
        &[r#"{"r":[{"d":9},[{"d":null}]]}"#],
        &[r#"{"x":[1,[2]]}"#],
        &[r#"{"x":[{"a":1},[{"a":2}]]}"#],
        &[r#"{"x":[[true,{"d":null}],1]}"#],
        &[r#"{"t":[1]}"#, r#"{"t":null}"#, r#"{"t":2}"#],
        &[r#"{"t":{"a":1}}"#, r#"{"t":null}"#, r#"{"t":[{"a":2}]}"#],
        &[r#"{"t":[1]}"#, r#"{"t":{"a":1}}"#, r#"{"t":null}"#],
    ];
    for rows in cases {
        let (schema, normalised) = normalised_against_inferred(rows, true);
        let field = &schema["fields"][0]["type"];
        let array = match field.as_array().map(Vec::as_slice) {
            Some([a, null] | [null, a]) if null == "null" => a,
            _ => field,
        };
        assert_eq!(array["type"], "array", "{rows:?}: {field}");
        for (row, out) in rows.iter().zip(&normalised) {
            let mut counts = std::collections::HashMap::new();
            count_scalars(&serde_json::from_str(row).unwrap(), &mut counts, 1);
            count_scalars(out, &mut counts, -1);
            assert!(counts.values().all(|&c| c == 0), "{row} -> {out}");
        }
    }
    // #238: the integer, the record's `d` and the booleans each land once
    let (_, normalised) = normalised_against_inferred(cases[0], true);
    assert_eq!(normalised[0]["r"][1][0]["__integer"], 1);
    assert_eq!(normalised[0]["r"][2][0]["d"], 9);
    assert_eq!(normalised[1]["r"][0][0]["__boolean"], true);
    // Either order of the branches gives the one schema
    assert_eq!(
        normalised_against_inferred(cases[1], true).0,
        normalised_against_inferred(cases[2], true).0
    );
}

/// A union of an array with scalars of other kinds than its items', or with a branch that
/// is itself a union of types, is one array holding them all, each kind in a field of its
/// own; with `wrap_scalars` off it stays a union, as before.
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_union_of_an_array_and_other_kinds_holds_every_value() {
    use crate::SchemaInferenceConfig;
    for rows in [
        &[r#"{"x":[[1],"a"]}"#][..],
        &[r#"{"u":-4}"#, r#"{"u":[]}"#, r#"{"u":{}}"#],
    ] {
        let (schema, normalised) = normalised_against_inferred(rows, true);
        assert!(!schema.to_string().contains("anyOf"), "{rows:?}: {schema}");
        for (row, out) in rows.iter().zip(&normalised) {
            let mut counts = std::collections::HashMap::new();
            count_scalars(&serde_json::from_str(row).unwrap(), &mut counts, 1);
            count_scalars(out, &mut counts, -1);
            assert!(counts.values().all(|&c| c == 0), "{row} -> {out}");
        }
    }
    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        wrap_scalars: false,
        ..Default::default()
    };
    let rows = [r#"{"r":[[{"d":null}],{"d":9}]}"#];
    let schema = crate::infer_json_schema_from_strings(&rows, config).unwrap().schema;
    assert!(schema.to_string().contains("anyOf"), "{rows:?}: {schema}");
}

/// For random JSON text (valid, invalid and missing rows), schemas inferred from the valid
/// rows (wrapped or not) and every option and map encoding: `normalise_to_json` gives each
/// row's text as `serde_json::to_string` gives it for the `Value` path, the typed decoder
/// taking the rows in most cases.
#[cfg(feature = "parquet")]
#[test]
fn test_normalise_to_json_matches_the_value_path() {
    use crate::{infer_json_schema_from_strings, SchemaInferenceConfig};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let mut rng = Rng(0x94d0_49bb_1331_11eb);
    let configs = normalise_configs();
    let (mut compared, mut typed_cases) = (0, 0);
    for case in 0..2500 {
        let wrap = case % 5 != 4;
        let mut rows: Vec<Option<String>> = Vec::new();
        for _ in 0..1 + rng.below(8) {
            let mut text = String::new();
            if !wrap && rng.below(8) != 0 {
                text.push_str("{\"a\":");
                random_text(&mut rng, 3, &mut text);
                text.push_str(",\"v\":");
                random_text(&mut rng, 3, &mut text);
                text.push('}');
            } else {
                random_text(&mut rng, 4, &mut text);
            }
            let half = (0..=text.len() / 2)
                .rev()
                .find(|&i| text.is_char_boundary(i))
                .unwrap_or(0);
            let text = match rng.below(14) {
                0 => text[..half].to_string(),
                1 => format!("{text} x"),
                _ => text,
            };
            rows.push(if rng.below(15) == 0 { None } else { Some(text) });
        }
        let strings: Vec<String> = rows
            .iter()
            .flatten()
            .filter_map(|r| serde_json::from_str::<Value>(r).ok())
            .map(|v| if wrap { json!({ "w": v }) } else { v })
            .map(|v| v.to_string())
            .collect();
        if strings.is_empty() {
            continue;
        }
        let config = SchemaInferenceConfig {
            avro: true,
            map_threshold: [20, 2, 0][case % 3],
            unify_maps: case % 2 == 0,
            ..Default::default()
        };
        let Ok(inferred) = infer_json_schema_from_strings(&strings, config) else {
            continue;
        };
        if inferred.schema.get("type") != Some(&json!("record")) {
            continue;
        }
        let cfg = NormaliseConfig {
            wrap_root: wrap.then(|| "w".to_string()),
            ..configs[case % configs.len()].clone()
        };
        let plan = NormalisePlan::new(&inferred.schema, &cfg, &HashSet::new());
        let borrowed: Vec<Option<&str>> = rows.iter().map(Option::as_deref).collect();
        let reference = catch_unwind(AssertUnwindSafe(|| {
            borrowed
                .iter()
                .map(|row| {
                    let value = row
                        .and_then(|row| serde_json::from_str(row).ok())
                        .unwrap_or(Value::Null);
                    serde_json::to_string(&plan.normalise(value).0).unwrap()
                })
                .collect::<Vec<String>>()
        }));
        // Where the `Value` path panics, so does its fallback here
        let Ok(reference) = reference else {
            continue;
        };
        let native = cfg.map_encoding == MapEncoding::Mapping;
        let fields = crate::parquet::avro_record_fields(&inferred.schema, native).ok();
        let typed = fields.as_ref().and_then(|f| plan.to_arrow(f)?.normalise(&borrowed, 0));
        typed_cases += usize::from(typed.is_some());
        let json = normalise_to_json(&plan, fields.as_ref(), &borrowed);
        assert_eq!(json, reference, "rows {rows:?}\nschema {}\ncfg {cfg:?}", inferred.schema);
        compared += 1;
    }
    eprintln!("normalise_to_json: {compared} compared, {typed_cases} typed");
    // The typed decoder declines the entries encoding, among the configs
    assert!(compared > 1500 && typed_cases * 2 > compared, "{compared} compared, {typed_cases} typed");
}

/// The rows decoded strictly against `schema` (`to_arrow_strict`), if they all fit it.
#[cfg(all(feature = "avro", feature = "parquet"))]
fn strict_decoded(schema: &Value, rows: &[&str]) -> Option<arrow::array::StructArray> {
    let cfg = NormaliseConfig::default();
    let plan = NormalisePlan::new(schema, &cfg, &HashSet::new());
    let fields = crate::parquet::avro_record_fields(schema, false).unwrap();
    let borrowed: Vec<Option<&str>> = rows.iter().copied().map(Some).collect();
    let strict = plan.to_arrow_strict(&fields)?.normalise(&borrowed, 0)?.0;
    // As the plan decodes them when not strict
    let (loose, _) = plan.to_arrow(&fields).unwrap().normalise(&borrowed, 0).unwrap();
    assert_eq!(strict, loose, "{schema}");
    Some(strict)
}

#[cfg(all(feature = "avro", feature = "parquet"))]
fn inferred(rows: &[&str]) -> Value {
    let config = crate::SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        avro: true,
        ..Default::default()
    };
    crate::infer_json_schema_from_strings(rows, config).unwrap().schema
}

/// Rows whose own inferred schema the schema holds decode strictly, as they do when not
/// strict: fields they lack, an integer where a double is, an empty array, nulls.
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_strict_decodes_rows_that_fit() {
    let schema = inferred(&[
        r#"{"s":"x","i":1,"f":1.5,"b":true,"l":[1,2],"r":{"a":"b"},"z":null}"#,
        r#"{"s":null,"l":[3]}"#,
    ]);
    let rows = [
        r#"{"s":"y","i":2,"f":2,"l":[],"r":{"a":"c"}}"#,
        r#"{"z":null,"r":{}}"#,
        r#"{}"#,
        r#"{"s":null,"i":null,"f":null,"b":false,"l":null,"r":null}"#,
    ];
    let decoded = strict_decoded(&schema, &rows).expect("the rows fit");
    assert_eq!(decoded.len(), 4);
}

/// A row with anything its schema would not already hold is not decoded strictly: a
/// member it has no field for (at the root or nested), a value of another type, a scalar
/// in place of an array or a record, a value where it saw only nulls.
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_strict_rejects_rows_that_do_not_fit() {
    let schema = inferred(&[
        r#"{"s":"x","i":1,"f":1.5,"b":true,"l":[1,2],"r":{"a":"b"},"z":null}"#,
    ]);
    assert!(strict_decoded(&schema, &[r#"{"s":"y"}"#]).is_some());
    for misfit in [
        r#"{"q":1}"#,
        r#"{"r":{"a":"b","c":1}}"#,
        r#"{"s":1}"#,
        r#"{"s":true}"#,
        r#"{"i":1.5}"#,
        r#"{"i":"1"}"#,
        r#"{"f":"1.5"}"#,
        r#"{"b":1}"#,
        r#"{"l":3}"#,
        r#"{"l":[1.5]}"#,
        r#"{"r":"b"}"#,
        r#"{"r":[]}"#,
        r#"{"z":0}"#,
    ] {
        assert!(
            strict_decoded(&schema, &[r#"{"s":"y"}"#, misfit]).is_none(),
            "{misfit} fits {schema}"
        );
    }
}

/// A union of two or more types is normalised as its first branch, so it is not
/// compiled strictly: a value of another branch infers to another schema.
#[cfg(all(feature = "avro", feature = "parquet"))]
#[test]
fn test_strict_does_not_compile_a_union() {
    let schema = serde_json::json!({
        "type": "record",
        "name": "document",
        "fields": [{"name": "u", "type": ["null", "string", "long"]}]
    });
    let cfg = NormaliseConfig::default();
    let plan = NormalisePlan::new(&schema, &cfg, &HashSet::new());
    if let Ok(fields) = crate::parquet::avro_record_fields(&schema, false) {
        assert!(plan.to_arrow_strict(&fields).is_none());
    }
}
