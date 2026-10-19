use super::OnDag;
use crate::dag::Dag;
use crate::genson_rs::{build_single_json_object_schema, get_builder};
use crate::schema::{
    infer_json_schema_from_strings_by, preprocess_force_field_types, rewrite_objects, Rewrite,
    SchemaInferenceConfig,
};
use serde_json::{json, Map, Value};

/// The cases of a test: all of them optimised, a quarter in a debug build, which runs each
/// many times slower.
fn cases(n: usize) -> usize {
    if cfg!(debug_assertions) {
        n / 4
    } else {
        n
    }
}

/// A small deterministic generator (xorshift), so the cases are the same on every run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }

    fn one<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

/// Field names: some that the configs below force, promote or keep from unifying, some
/// that are keywords, and some made only of digits.
const NAMES: [&str; 14] = [
    "a", "b", "k1", "k2", "k3", "k4", "value", "datavalue", "mainsnak", "qualifiers", "type",
    "properties", "anyOf", "7",
];
const SCALARS: [&str; 5] = ["string", "integer", "number", "boolean", "null"];

fn scalar(rng: &mut Rng) -> Value {
    match rng.below(9) {
        0 => json!({ "type": ["null", rng.one(&SCALARS[..4])] }),
        1 => json!({ "type": [rng.one(&SCALARS[..4]), "null"] }),
        2 => json!({ "type": ["integer", "string"] }),
        3 => json!({ "type": "number", "maximum": 2.5, "minimum": -0.0 }),
        _ => json!({ "type": rng.one(&SCALARS) }),
    }
}

/// A record over `names`, its fields from `field`, with some required and its keywords in
/// some order.
fn record(rng: &mut Rng, names: &[String], field: &mut dyn FnMut(&mut Rng) -> Value) -> Value {
    let mut properties = Map::new();
    for name in names {
        properties.insert(name.clone(), field(rng));
    }
    let required: Vec<Value> = match rng.below(4) {
        0 => properties.keys().map(|k| json!(k)).collect(),
        1 => vec![],
        _ => properties
            .keys()
            .filter(|_| rng.chance(2))
            .map(|k| json!(k))
            .collect(),
    };
    let t = match rng.below(8) {
        0 => json!(["null", "object"]),
        1 => json!(["object", "null"]),
        _ => json!("object"),
    };
    let mut record = Map::new();
    let keywords_first = rng.chance(5);
    if keywords_first {
        record.insert("properties".into(), Value::Object(properties.clone()));
    }
    record.insert("type".into(), t);
    if !keywords_first {
        record.insert("properties".into(), Value::Object(properties));
    }
    if !required.is_empty() || rng.chance(6) {
        record.insert("required".into(), Value::Array(required));
    }
    match rng.below(12) {
        0 => drop(record.insert("description".into(), json!("d"))),
        1 => drop(record.insert("name".into(), json!({ "type": "object", "properties": {} }))),
        2 => drop(record.insert("additionalProperties".into(), scalar(rng))),
        3 => drop(record.insert("additionalProperties".into(), json!(true))),
        4 => drop(record.insert("$schema".into(), json!("urn:x"))),
        _ => {}
    }
    Value::Object(record)
}

fn names(rng: &mut Rng) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for _ in 0..1 + rng.below(4) {
        let name = rng.one(&NAMES).to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Any schema, adversarial ones included: legacy nullable wrappers, `anyOf`s, maps with
/// properties too, tuples, empty records and items, schemas without a type.
fn schema(rng: &mut Rng, depth: u32) -> Value {
    let pick = if depth == 0 { rng.below(3) } else { rng.below(22) };
    match pick {
        0..=2 => scalar(rng),
        3..=8 => {
            let names = names(rng);
            record(rng, &names, &mut |rng| schema(rng, depth - 1))
        }
        9 | 10 => {
            let items = schema(rng, depth - 1);
            match rng.below(6) {
                0 => json!({ "type": "array" }),
                1 => json!({ "type": "array", "items": {} }),
                2 => json!({ "type": ["null", "array"], "items": items }),
                3 => json!({ "type": "array", "items": [items, scalar(rng)] }),
                _ => json!({ "type": "array", "items": items }),
            }
        }
        11 => json!({ "type": "object", "additionalProperties": schema(rng, depth - 1) }),
        12 => [
            json!({ "type": "object" }),
            json!({ "type": "object", "properties": {} }),
            json!({ "type": ["null", "object"], "properties": {}, "required": [] }),
        ][rng.below(3) as usize]
            .clone(),
        13 => match rng.below(2) {
            0 => json!(["null", schema(rng, depth - 1)]),
            _ => json!([schema(rng, depth - 1), "null"]),
        },
        14 | 15 => {
            let branches: Vec<Value> = (0..2 + rng.below(2)).map(|_| schema(rng, depth - 1)).collect();
            json!({ "anyOf": branches })
        }
        16 => json!({ "anyOf": [scalar(rng), schema(rng, depth - 1)] }),
        17 => [json!({}), json!({ "description": "x" })][rng.below(2) as usize].clone(),
        18..=20 => map_candidate(rng, depth - 1),
        _ => json!({ "type": "object", "properties": { "value__string": { "type": "string" } } }),
    }
}

/// `schema` changed a little, so that the changed and the unchanged unify, or nearly.
fn vary(rng: &mut Rng, schema: &Value, depth: u32) -> Value {
    let mut varied = schema.clone();
    match rng.below(10) {
        0 => return scalar(rng),
        1 => return json!({ "type": "null" }),
        2 => return json!({ "type": "array", "items": schema }),
        3 => return json!(["null", schema]),
        _ => {}
    }
    let Some(obj) = varied.as_object_mut() else {
        return varied;
    };
    match rng.below(6) {
        0 => {
            // Nullable, or not
            if let Some(Value::String(t)) = obj.get("type").cloned() {
                obj.insert("type".into(), json!(["null", t]));
            } else if let Some(Value::Array(ts)) = obj.get("type").cloned() {
                if let Some(t) = ts.iter().find(|t| *t != "null") {
                    obj.insert("type".into(), t.clone());
                }
            }
        }
        1 => {
            // The keywords in another order
            let mut entries: Vec<(String, Value)> = std::mem::take(obj).into_iter().collect();
            entries.reverse();
            obj.extend(entries);
        }
        _ => {
            if let Some(Value::Object(properties)) = obj.get_mut("properties") {
                match rng.below(4) {
                    0 if !properties.is_empty() => {
                        let at = rng.below(properties.len() as u64) as usize;
                        let name = properties.keys().nth(at).unwrap().clone();
                        properties.shift_remove(&name);
                    }
                    1 => {
                        let name = rng.one(&NAMES).to_string();
                        properties.insert(name, self::schema(rng, depth));
                    }
                    _ if !properties.is_empty() => {
                        let at = rng.below(properties.len() as u64) as usize;
                        let name = properties.keys().nth(at).unwrap().clone();
                        let field = properties.get_mut(&name).unwrap();
                        *field = vary(rng, &field.clone(), depth);
                    }
                    _ => {}
                }
            } else if let Some(items) = obj.get_mut("items") {
                *items = vary(rng, &items.clone(), depth);
            }
        }
    }
    varied
}

/// An object of many fields from one family of schemas: identical, alike once nullability
/// is set aside, unifiable, or not.
fn map_candidate(rng: &mut Rng, depth: u32) -> Value {
    let base = schema(rng, depth);
    let family: Vec<Value> = (0..1 + rng.below(4)).map(|_| vary(rng, &base, depth)).collect();
    let n = [2, 3, 5, 12, 25, 60][rng.below(6) as usize];
    let names: Vec<String> = (0..n)
        .map(|i| match rng.below(10) {
            0 => rng.one(&NAMES).to_string(),
            _ => format!("P{i}"),
        })
        .collect();
    let mut field = |rng: &mut Rng| match rng.below(4) {
        0 => base.clone(),
        1 => vary(rng, &base, depth),
        _ => rng.one(&family).clone(),
    };
    record(rng, &names, &mut field)
}

/// A config with a random mix of the options that steer map inference and unification.
fn random_config(rng: &mut Rng) -> SchemaInferenceConfig {
    let subset = |rng: &mut Rng, names: &[&str]| -> std::collections::HashSet<String> {
        names
            .iter()
            .filter(|_| rng.chance(3))
            .map(|s| s.to_string())
            .collect()
    };
    let forced = |rng: &mut Rng, pairs: &[(&str, &str)]| -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .filter(|_| rng.chance(4))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    SchemaInferenceConfig {
        map_threshold: *rng.one(&[0, 0, 1, 2, 3, 5, 20]),
        map_max_required_keys: *rng.one(&[None, None, Some(0), Some(1), Some(3)]),
        unify_maps: !rng.chance(4),
        wrap_scalars: !rng.chance(3),
        no_root_map: !rng.chance(3),
        force_field_types: forced(
            rng,
            &[("mainsnak", "record"), ("qualifiers", "map"), ("k1", "map"), ("k2", "record"), ("a", "other")],
        ),
        force_parent_field_types: forced(rng, &[("k3", "record"), ("k4", "map")]),
        force_scalar_promotion: subset(rng, &["datavalue", "value", "k2", "mainsnak"]),
        no_unify: subset(rng, &["qualifiers", "k4", "P3"]),
        ..Default::default()
    }
}

/// `rewrite_objects` as inference runs it, `Value`s rewritten in place, on `threads`.
fn rewrite_with_values(schema: &Value, config: &SchemaInferenceConfig, threads: usize) -> String {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let mut schema = schema.clone();
        if config.force_field_types.values().any(|t| t == "map") {
            preprocess_force_field_types(&mut schema, config);
        }
        rewrite_objects(&mut schema, None, config, true);
        schema.to_string()
    })
}

fn rewrite_on_dag(schema: &Value, config: &SchemaInferenceConfig) -> String {
    let mut dag = Dag::new();
    let mut id = dag.intern_value(schema);
    let mut on = OnDag::new(dag, config);
    if config.force_field_types.values().any(|t| t == "map") {
        id = on.preprocess_force_field_types(id);
    }
    let id = on.rewrite_objects(id, None, true);
    on.dag.to_value(id).to_string()
}

/// For random schemas under random configs, the rewrite on a DAG gives the schema (key order
/// included) that `rewrite_objects` gives in place, on one thread or four; and most of the
/// schemas are rewritten, many of them into maps, so that the comparisons are of rewrites.
#[test]
fn test_rewrite_on_dag_matches_rewrite_objects() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut changed, mut mapped) = (0, 0);
    let cases = cases(3000);
    for case in 0..cases {
        let config = random_config(&mut rng);
        let root = match rng.below(5) {
            0 => schema(&mut rng, 4),
            1 => map_candidate(&mut rng, 3),
            _ => {
                let names = names(&mut rng);
                let mut root = record(&mut rng, &names, &mut |rng| match rng.below(3) {
                    0 => map_candidate(rng, 3),
                    _ => schema(rng, 3),
                });
                if let Some(obj) = root.as_object_mut() {
                    obj.shift_insert(0, "$schema".into(), json!("http://json-schema.org/schema#"));
                }
                root
            }
        };
        let threads = if case % 2 == 0 { 1 } else { 4 };
        let by_values = rewrite_with_values(&root, &config, threads);
        let on_dag = rewrite_on_dag(&root, &config);
        assert_eq!(by_values, on_dag, "case {case}: {root}\nconfig {config:?}");
        let input = root.to_string();
        changed += usize::from(by_values != input);
        let maps = |s: &str| s.matches("additionalProperties").count();
        mapped += usize::from(maps(&by_values) > maps(&input));
    }
    assert!(changed > cases / 2 && mapped > cases / 4, "{changed} changed, {mapped} mapped");
}

/// Random JSON for the row phase, as `random_document` in the schema tests, less hostile.
fn document(rng: &mut Rng, depth: u32, out: &mut String) {
    let pick = if depth == 0 { 3 + rng.below(5) } else { rng.below(8) };
    match pick {
        0 | 1 => {
            out.push('{');
            let n = if rng.chance(8) { 15 + rng.below(10) } else { rng.below(5) };
            for i in 0..n {
                if i > 0 {
                    out.push(',');
                }
                let key = rng.one(&NAMES).to_string();
                let key = if n > 12 { format!("{key}{}", rng.below(40)) } else { key };
                out.push_str(&format!("{key:?}:"));
                document(rng, depth - 1, out);
            }
            out.push('}');
        }
        2 => {
            out.push('[');
            for i in 0..rng.below(4) {
                if i > 0 {
                    out.push(',');
                }
                document(rng, depth - 1, out);
            }
            out.push(']');
        }
        3 => out.push_str("null"),
        4 => out.push_str(rng.one(&["true", "false"])),
        5 => out.push_str(rng.one(&["0", "-7", "1.5", "1e2"])),
        _ => out.push_str(rng.one(&["\"s\"", "\"\"", "\"\\u00e9\""])),
    }
}

/// For builders of random rows and random schemas, `to_dag` gives the schema `to_schema`
/// gives, key order included, whatever the builder's `$schema`.
#[test]
fn test_to_dag_matches_to_schema() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let (mut compared, n) = (0, cases(600));
    for case in 0..n {
        let uri = [None, Some("NULL"), Some("AUTO"), Some("urn:x")][case % 4];
        let mut inputs: Vec<Result<Value, String>> = Vec::new();
        for _ in 0..1 + rng.below(12) {
            if rng.chance(3) {
                inputs.push(Ok(schema(&mut rng, 3)));
            } else {
                let mut row = String::new();
                document(&mut rng, 4, &mut row);
                if row.starts_with('{') {
                    inputs.push(Err(row));
                }
            }
        }
        // A schema genson cannot take (a legacy wrapper, say) panics, as it does in
        // inference; such a case is left out
        let built = std::panic::catch_unwind(|| {
            let mut builder = get_builder(uri);
            for input in inputs {
                match input {
                    Ok(schema) => builder.add_schema(schema),
                    Err(row) => build_single_json_object_schema(&mut builder, &mut row.into_bytes()),
                }
            }
            builder
        });
        let Ok(builder) = built else { continue };
        compared += 1;
        let mut dag = Dag::new();
        let id = builder.to_dag(&mut dag);
        assert_eq!(
            dag.to_value(id).to_string(),
            builder.to_schema().to_string(),
            "case {case}"
        );
    }
    assert!(compared > n * 2 / 3, "{compared} of {n} compared");
}

/// A statement as Wikidata writes them, its value's kind set by its property (or, now and
/// then, not), with qualifiers and references of other properties.
fn statement(rng: &mut Rng, property: u64, out: &mut String) {
    let value = |rng: &mut Rng, property: u64| -> String {
        let kind = if rng.chance(25) { rng.below(5) } else { property % 5 };
        match kind {
            0 => "\"s\"".to_string(),
            1 => format!("{{\"time\":\"+2001\",\"precision\":{}}}", rng.below(12)),
            2 => format!("{{\"id\":\"Q{0}\",\"numeric-id\":{0}}}", rng.below(99)),
            3 => "{\"amount\":\"+1\",\"unit\":\"1\"}".to_string(),
            _ => format!("{{\"latitude\":1.5,\"longitude\":{},\"precision\":0.1}}", rng.below(3)),
        }
    };
    let snak = |rng: &mut Rng, property: u64| -> String {
        if rng.chance(8) {
            format!("{{\"snaktype\":\"somevalue\",\"property\":\"P{property}\"}}")
        } else {
            format!(
                "{{\"snaktype\":\"value\",\"property\":\"P{property}\",\"datavalue\":{{\"value\":{},\"type\":\"t\"}}}}",
                value(rng, property)
            )
        }
    };
    out.push_str(&format!("{{\"mainsnak\":{},\"type\":\"statement\"", snak(rng, property)));
    if rng.chance(2) {
        let qualifiers: Vec<String> = (0..1 + rng.below(3))
            .map(|_| {
                let q = rng.below(30);
                format!("\"P{q}\":[{}]", snak(rng, q))
            })
            .collect();
        out.push_str(&format!(",\"qualifiers\":{{{}}}", qualifiers.join(",")));
    }
    if rng.chance(3) {
        let r = rng.below(30);
        out.push_str(&format!(
            ",\"references\":[{{\"hash\":\"h\",\"snaks\":{{\"P{r}\":[{}]}}}}]",
            snak(rng, r)
        ));
    }
    out.push_str(",\"rank\":\"normal\"}");
}

/// Rows of claims (each an object of properties' statements), or of any JSON.
fn claims_row(rng: &mut Rng) -> String {
    let mut row = String::new();
    if rng.chance(10) {
        document(rng, 4, &mut row);
        if !row.starts_with('{') {
            row = "{}".to_string();
        }
        return row;
    }
    row.push('{');
    let properties: Vec<u64> = (0..1 + rng.below(6)).map(|_| rng.below(60)).collect();
    let mut seen = Vec::new();
    for property in properties {
        if seen.contains(&property) {
            continue;
        }
        if !seen.is_empty() {
            row.push(',');
        }
        seen.push(property);
        row.push_str(&format!("\"P{property}\":["));
        for i in 0..1 + rng.below(3) {
            if i > 0 {
                row.push(',');
            }
            statement(rng, property, &mut row);
        }
        row.push(']');
    }
    row.push('}');
    row
}

/// wikidata-pq's config for claims.
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
        ..Default::default()
    }
}

/// For random rows of claims (and of other JSON) under wikidata-pq's config, the defaults
/// and random configs, at several chunk sizes and thread counts, inference gives the same
/// result (schema, key order included, processed count, or error) whichever way the merged
/// schema is rewritten.
#[test]
fn test_inference_on_dag_matches_values() {
    let mut rng = Rng(0x6a09_e667_f3bc_c908);
    let (mut compared, n) = (0, cases(240));
    for case in 0..n {
        let rows: Vec<String> = (0..5 + rng.below(40)).map(|_| claims_row(&mut rng)).collect();
        let mut config = match case % 3 {
            0 => claims_config(),
            1 => SchemaInferenceConfig::default(),
            _ => random_config(&mut rng),
        };
        config.max_builders = [None, Some(1), Some(3)][rng.below(3) as usize];
        #[cfg(feature = "avro")]
        {
            config.avro = rng.chance(4);
        }
        if case % 3 == 2 && rng.chance(2) {
            config.delimiter = Some(b'\n');
            config.wrap_root = Some("claims".to_string());
        }
        let threads = [1, 2, 4][rng.below(3) as usize];
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let (by_values, on_dag) = pool.install(|| {
            let infer = |way| {
                infer_json_schema_from_strings_by(&rows, config.clone(), way)
                    .map(|result| (result.schema.to_string(), result.processed_count))
            };
            (infer(Rewrite::Values), infer(Rewrite::Dag))
        });
        compared += usize::from(by_values.is_ok());
        assert_eq!(by_values, on_dag, "case {case}: {threads} threads, config {config:?}");
    }
    assert!(compared > n * 5 / 6, "{compared} of {n} compared");
}
