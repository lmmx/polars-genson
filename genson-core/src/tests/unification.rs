use super::*;
use serde_json::json;
use crate::{infer_json_schema_from_strings, schema::rewrite_objects};

#[test]
fn test_scalar_unification_ndjson_mixed_nullable_formats() {
    let ndjson_input = r#"
{"theme": {"red": {"colors": {"primary": "ff"}}, "blue": {"colors": {"secondary": "00"}}}}
{"theme": {"green": {"colors": {"accent": "cc"}}, "red": {"colors": {"secondary": "aa"}}}}
{"theme": {"blue": {"brightness": 255}}}
"#;

    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 1,
        unify_maps: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&[ndjson_input.to_string()], config)
        .expect("Should handle NDJSON with nested maps triggering scalar unification");
    println!("Generated schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    // Navigate to the colors field that should have been unified
    let themes_schema = &result.schema["properties"]["theme"];
    assert!(themes_schema.get("additionalProperties").is_some());

    let theme_record = &themes_schema["additionalProperties"];
    let colors_schema = &theme_record["properties"]["colors"];

    // Should be converted to map due to scalar unification
    assert!(colors_schema.get("additionalProperties").is_some());
    assert!(colors_schema.get("properties").is_none());

    // The unified type should be nullable string (some colors components missing from some themes)
    let colors_values = &colors_schema["additionalProperties"];
    assert_eq!(colors_values["type"], json!("string"));
}

#[test]
fn test_scalar_unification_with_old_nullable_format() {
    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        ..Default::default()
    };

    // Simulate the old nullable format that was causing issues
    let schemas = [
        json!({"type": "string"}),                           // Regular string
        json!({"type": ["null", "string"]}),                 // New nullable format
        json!(["null", {"type": ["null", "string"]}]),       // Old nullable format
    ];

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = check_unifiable_schemas(&schema_refs, "test", &config);
    
    // Should successfully unify all scalar string types
    assert!(result.is_some());
    let unified = result.unwrap();
    assert_eq!(unified["type"], json!(["null", "string"]));
}

#[test]
fn test_is_scalar_schema_with_mixed_formats() {
    // Test the updated is_scalar_schema function
    assert!(is_scalar_schema(&json!({"type": "string"})));
    assert!(is_scalar_schema(&json!({"type": ["null", "string"]})));
    assert!(is_scalar_schema(&json!(["null", {"type": "string"}])));
    assert!(is_scalar_schema(&json!(["null", {"type": ["null", "string"]}])));
    
    // Should reject object types
    assert!(!is_scalar_schema(&json!({"type": "object", "properties": {}})));
    assert!(!is_scalar_schema(&json!({"type": "array", "items": {}})));
}

#[test]
fn test_anyof_unification() {
    let mut anyof_schema = json!({
        "anyOf": [
            {"type": "object", "properties": {"timezone": {"type": "integer"}}},
            {"type": "string"}
        ]
    });

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    rewrite_objects(&mut anyof_schema, Some("datavalue"), &config, false);
    println!("Generated schema: {}", serde_json::to_string_pretty(&anyof_schema).unwrap());

    // Should be unified to a single object, not anyOf
    assert!(anyof_schema.get("anyOf").is_none(), "anyOf should be unified away");
    assert_eq!(anyof_schema["type"], "object");
    assert!(anyof_schema.get("properties").is_some(), "Should have properties after unification");
}

#[test]
fn test_scalar_vs_mixed_type_object_unification() {
    let test_data = vec![
        json!({"datavalue": "7139c051-8ea3-3f93-8bbc-6e7dff6d61a4"}).to_string(),
        json!({"datavalue": {"timezone": 0, "precision": 11}}).to_string(),
        json!({"datavalue": {"id": "Q1022293", "labels": {"ru": "до мажор"}}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config)
        .expect("Should succeed with scalar promotion and record unification");
    println!("Generated schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["datavalue"];
    assert_eq!(datavalue_schema["type"], "object");

    // Should have properties (record) not additionalProperties (map) due to mixed value types
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema.get("additionalProperties").is_none());
}

#[test]
fn test_anyof_rewrite_objects_root() {
    let mut nested_schema = json!({
        "type": "object",
        "properties": {
            "claims": {
                "type": "object",
                "additionalProperties": {
                    "anyOf": [
                        {"type": "object", "properties": {"timezone": {"type": "integer"}}},
                        {"type": "string"}
                    ]
                }
            }
        }
    });

    let config = SchemaInferenceConfig { unify_maps: true, wrap_scalars: true, ..Default::default() };
    rewrite_objects(&mut nested_schema, None, &config, true);

    println!("{}", nested_schema);

    // Should not have anyOf anymore
    assert!(nested_schema["properties"]["claims"]["additionalProperties"].get("anyOf").is_none());
}

#[test]
fn test_anyof_rewrite_objects_nested() {
    let mut nested_schema = json!({
        "type": "object",
        "properties": {
            "claims": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": {
                        "anyOf": [
                            {"type": "object", "properties": {"timezone": {"type": "integer"}}},
                            {"type": "string"}
                        ]
                    }
                }
            }
        }
    });

    let config = SchemaInferenceConfig { unify_maps: true, wrap_scalars: true, ..Default::default() };
    rewrite_objects(&mut nested_schema, None, &config, true);

    println!("{}", nested_schema);

    // This currently fails - anyOf is still there
    let inner_schema = &nested_schema["properties"]["claims"]["additionalProperties"]["additionalProperties"];
    assert!(inner_schema.get("anyOf").is_none(), "anyOf should be unified away");
    assert_eq!(inner_schema["type"], "object");
}

#[test]
fn test_schema_inference_root_anyof() {
    let test_data = vec![
        json!({"datavalue": "string-value"}).to_string(),
        json!({"datavalue": {"timezone": 0}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config).unwrap();
    println!("Root anyOf schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["datavalue"];
    assert_eq!(datavalue_schema["type"], "object");
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema["properties"].get("datavalue__string").is_some());
}

#[test]
fn test_schema_inference_nested_anyof() {
    let test_data = vec![
        json!({"claims": {"P31": {"datavalue": "string-value"}}}).to_string(),
        json!({"claims": {"P31": {"datavalue": {"timezone": 0}}}}).to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        wrap_scalars: true,
        debug: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&test_data, config).unwrap();
    println!("Nested anyOf schema: {}", serde_json::to_string_pretty(&result.schema).unwrap());

    let datavalue_schema = &result.schema["properties"]["claims"]["additionalProperties"]["additionalProperties"];
    assert_eq!(datavalue_schema["type"], "object");
    assert!(datavalue_schema.get("properties").is_some());
    assert!(datavalue_schema["properties"].get("datavalue__string").is_some());
}

#[test]
fn test_scalar_promotion_integer_vs_number() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "foo": {"type": "integer"}
            },
            "required": ["foo"]
        }),
        json!({
            "type": "object",
            "properties": {
                "foo": {"type": "number"}
            },
            "required": ["foo"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    assert_eq!(unified["type"], "object");

    let props = unified["properties"].as_object().expect("Should have properties");
    assert!(props.contains_key("foo"), "Should have foo field");

    let foo_schema = &props["foo"];
    assert_eq!(foo_schema["type"], "object");

    let foo_props = foo_schema["properties"].as_object().expect("Foo should have properties");
    assert!(foo_props.contains_key("foo__integer"), "Missing promoted integer field");
    assert!(foo_props.contains_key("foo__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_string_vs_boolean() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "bar": {"type": "string"}
            },
            "required": ["bar"]
        }),
        json!({
            "type": "object",
            "properties": {
                "bar": {"type": "boolean"}
            },
            "required": ["bar"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let bar_props = props["bar"]["properties"].as_object().expect("Bar should have properties");

    assert!(bar_props.contains_key("bar__string"), "Missing promoted string field");
    assert!(bar_props.contains_key("bar__boolean"), "Missing promoted boolean field");
}

#[test]
fn test_scalar_promotion_string_vs_number() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "baz": {"type": "string"}
            },
            "required": ["baz"]
        }),
        json!({
            "type": "object",
            "properties": {
                "baz": {"type": "number"}
            },
            "required": ["baz"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let baz_props = props["baz"]["properties"].as_object().expect("Baz should have properties");

    assert!(baz_props.contains_key("baz__string"), "Missing promoted string field");
    assert!(baz_props.contains_key("baz__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_string_vs_integer() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "qux": {"type": "string"}
            },
            "required": ["qux"]
        }),
        json!({
            "type": "object",
            "properties": {
                "qux": {"type": "integer"}
            },
            "required": ["qux"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let qux_props = props["qux"]["properties"].as_object().expect("Qux should have properties");

    assert!(qux_props.contains_key("qux__string"), "Missing promoted string field");
    assert!(qux_props.contains_key("qux__integer"), "Missing promoted integer field");
}

#[test]
fn test_scalar_promotion_boolean_vs_number() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "xyz": {"type": "boolean"}
            },
            "required": ["xyz"]
        }),
        json!({
            "type": "object",
            "properties": {
                "xyz": {"type": "number"}
            },
            "required": ["xyz"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let xyz_props = props["xyz"]["properties"].as_object().expect("Xyz should have properties");

    assert!(xyz_props.contains_key("xyz__boolean"), "Missing promoted boolean field");
    assert!(xyz_props.contains_key("xyz__number"), "Missing promoted number field");
}

#[test]
fn test_scalar_promotion_boolean_vs_integer() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "abc": {"type": "boolean"}
            },
            "required": ["abc"]
        }),
        json!({
            "type": "object",
            "properties": {
                "abc": {"type": "integer"}
            },
            "required": ["abc"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let abc_props = props["abc"]["properties"].as_object().expect("Abc should have properties");

    assert!(abc_props.contains_key("abc__boolean"), "Missing promoted boolean field");
    assert!(abc_props.contains_key("abc__integer"), "Missing promoted integer field");
}

#[test]
fn test_scalar_promotion_three_types() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "string"}
            },
            "required": ["multi"]
        }),
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "number"}
            },
            "required": ["multi"]
        }),
        json!({
            "type": "object",
            "properties": {
                "multi": {"type": "boolean"}
            },
            "required": ["multi"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let multi_props = props["multi"]["properties"].as_object().expect("Multi should have properties");

    assert!(multi_props.contains_key("multi__string"), "Missing promoted string field");
    assert!(multi_props.contains_key("multi__number"), "Missing promoted number field");
    assert!(multi_props.contains_key("multi__boolean"), "Missing promoted boolean field");
}

#[test]
fn test_scalar_promotion_all_four_types() {
    let schemas = [
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "string"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "number"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "integer"}
            },
            "required": ["all"]
        }),
        json!({
            "type": "object",
            "properties": {
                "all": {"type": "boolean"}
            },
            "required": ["all"]
        }),
    ];

    let config = SchemaInferenceConfig {
        wrap_scalars: true,
        ..Default::default()
    };

    let schema_refs: Vec<&Value> = schemas.iter().collect();
    let result = unify_record_schemas(&schema_refs, "root", &config);
    assert!(result.is_some(), "Should unify records with scalar promotion");

    let unified = result.unwrap();
    let props = unified["properties"].as_object().expect("Should have properties");
    let all_props = props["all"]["properties"].as_object().expect("All should have properties");

    assert!(all_props.contains_key("all__string"), "Missing promoted string field");
    assert!(all_props.contains_key("all__number"), "Missing promoted number field");
    assert!(all_props.contains_key("all__integer"), "Missing promoted integer field");
    assert!(all_props.contains_key("all__boolean"), "Missing promoted boolean field");
}

#[test]
fn test_nullable_union_from_null_and_typed() {
    // Test null + integer -> nullable integer
    let null_schema = json!({"type": "null"});
    let int_schema = json!({"type": "integer"});

    let result = try_make_nullable_union(&null_schema, &int_schema);
    assert_eq!(result, Some(json!({"type": ["null", "integer"]})));

    // Test in reverse order
    let result = try_make_nullable_union(&int_schema, &null_schema);
    assert_eq!(result, None); // Should return None when first arg is not null

    // Test null + string -> nullable string
    let string_schema = json!({"type": "string"});
    let result = try_make_nullable_union(&null_schema, &string_schema);
    assert_eq!(result, Some(json!({"type": ["null", "string"]})));

    // Test with schema that has additional properties
    let complex_schema = json!({
        "type": "integer",
        "minimum": 0,
        "maximum": 100
    });
    let result = try_make_nullable_union(&null_schema, &complex_schema);
    assert_eq!(result, Some(json!({
        "type": ["null", "integer"],
        "minimum": 0,
        "maximum": 100
    })));

    // Test null + null -> None
    let result = try_make_nullable_union(&null_schema, &null_schema);
    assert_eq!(result, None);
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

const KEYS: [&str; 6] = ["a", "b", "c", "id", "value", "x__string"];

/// A random schema of the forms unification meets: scalars (nullable either way round),
/// null, records (nullable, with required fields, sometimes over many keys), arrays (with
/// items, empty ones or none), maps, empty records, legacy nullable wrappers, `anyOf`
/// unions and schemas without a type.
fn random_schema(rng: &mut Rng, depth: u32) -> Value {
    let scalar = |rng: &mut Rng| ["string", "integer", "number", "boolean"][rng.below(4) as usize];
    let pick = if depth == 0 { rng.below(3) } else { rng.below(20) };
    match pick {
        0 => json!({ "type": scalar(rng) }),
        1 => match rng.below(2) {
            0 => json!({ "type": ["null", scalar(rng)] }),
            _ => json!({ "type": [scalar(rng), "null"] }),
        },
        2 => json!({ "type": "null" }),
        3..=8 => {
            let many = rng.below(8) == 0;
            let mut properties = Map::new();
            for i in 0..if many { 12 } else { 1 + rng.below(4) } {
                let key = if many { format!("k{i}") } else { KEYS[rng.below(6) as usize].to_string() };
                properties.insert(key, random_schema(rng, depth - 1));
            }
            let required: Vec<Value> = properties
                .keys()
                .filter(|_| rng.below(2) == 0)
                .map(|k| json!(k))
                .collect();
            let mut record = json!({ "type": "object", "properties": properties });
            if rng.below(6) == 0 {
                record["type"] = json!(["null", "object"]);
            }
            if !required.is_empty() {
                record["required"] = Value::Array(required);
            }
            record
        }
        9..=12 => match rng.below(6) {
            0 => json!({ "type": "array" }),
            1 => json!({ "type": "array", "items": {} }),
            2 => json!({ "type": ["null", "array"], "items": random_schema(rng, depth - 1) }),
            _ => json!({ "type": "array", "items": random_schema(rng, depth - 1) }),
        },
        13 | 14 => json!({ "type": "object", "additionalProperties": random_schema(rng, depth - 1) }),
        15 => [json!({ "type": "object" }), json!({ "type": "object", "properties": {} })]
            [rng.below(2) as usize]
            .clone(),
        16 => json!(["null", random_schema(rng, depth - 1)]),
        17 => json!({ "anyOf": [{ "type": scalar(rng) }, random_schema(rng, depth - 1)] }),
        18 => json!({}),
        _ => json!({ "type": "object", "properties": { "value__string": { "type": "string" } } }),
    }
}

/// A schema like `schema`: its fields dropped, added, made nullable or required, and its
/// parts changed or replaced, each now and then.
fn mutate(rng: &mut Rng, schema: &Value, depth: u32) -> Value {
    if rng.below(3) == 0 {
        return schema.clone();
    }
    if rng.below(12) == 0 {
        return random_schema(rng, depth);
    }
    let mut schema = schema.clone();
    let Some(obj) = schema.as_object_mut() else {
        return schema;
    };
    if let Some(Value::Object(properties)) = obj.get_mut("properties") {
        let keys: Vec<String> = properties.keys().cloned().collect();
        for key in keys {
            match rng.below(8) {
                0 => {
                    properties.shift_remove(&key);
                }
                1 | 2 => {
                    let field = mutate(rng, &properties[&key], depth.saturating_sub(1));
                    properties.insert(key, field);
                }
                _ => {}
            }
        }
        if rng.below(4) == 0 {
            properties.insert(KEYS[rng.below(6) as usize].to_string(), random_schema(rng, depth.saturating_sub(1)));
        }
        let required: Vec<Value> = properties.keys().filter(|_| rng.below(2) == 0).map(|k| json!(k)).collect();
        if rng.below(2) == 0 {
            obj.shift_remove("required");
            if !required.is_empty() {
                obj.insert("required".into(), Value::Array(required));
            }
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(inner) = obj.get(key).cloned() {
            obj.insert(key.into(), mutate(rng, &inner, depth.saturating_sub(1)));
        }
    }
    match obj.get("type").cloned() {
        Some(Value::String(t)) if t != "null" && rng.below(5) == 0 => {
            obj.insert("type".into(), json!(["null", t]));
        }
        Some(Value::String(t)) if rng.below(8) == 0 && ["string", "integer"].contains(&t.as_str()) => {
            obj.insert("type".into(), json!(if t == "string" { "integer" } else { "string" }));
        }
        _ => {}
    }
    schema
}

/// The fold `unify_field_schemas_sequential` makes, every step taken the borrowed way.
fn borrowed_fold(schemas: &[&Value], config: &SchemaInferenceConfig) -> Option<Value> {
    let (&first, rest) = schemas.split_first()?;
    let mut unified = first.clone();
    for &new in rest {
        if unified != *new {
            unified = unify_field_pair(Cow::Borrowed(&unified), new, "f", "p.f", "p", config)?;
        }
    }
    Some(unified)
}

/// `unify_field_schemas_parallel` with its halves merged the borrowed way.
fn borrowed_halves(schemas: &[&Value], config: &SchemaInferenceConfig) -> Option<Value> {
    if schemas.len() < 10 {
        return borrowed_fold(schemas, config);
    }
    let (left, right) = schemas.split_at(schemas.len() / 2);
    let (left, right) = (borrowed_halves(left, config)?, borrowed_halves(right, config)?);
    check_unifiable_schemas(&[&left, &right], "p.f", config)
}

#[test]
fn test_owned_unification_matches_the_borrowed() {
    let text = |v: Option<Value>| v.map(|v| v.to_string());
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let (mut unified, mut cases) = (0, 0);
    for case in 0..3000 {
        let config = SchemaInferenceConfig {
            wrap_scalars: case % 3 != 0,
            ..Default::default()
        };
        let a = random_schema(&mut rng, 4);
        let b = mutate(&mut rng, &a, 4);
        // A pair, as `check_unifiable_schemas` (every step of which is borrowed) unifies it
        let borrowed = check_unifiable_schemas(&[&a, &b], "p", &config);
        unified += borrowed.is_some() as usize;
        assert_eq!(text(unify_pair(a.clone(), &b, "p", &config)), text(borrowed), "{a}\n{b}");
        if a != b {
            assert_eq!(
                text(unify_field_pair(Cow::Owned(a.clone()), &b, "f", "p.f", "p", &config)),
                text(unify_field_pair(Cow::Borrowed(&a), &b, "f", "p.f", "p", &config)),
                "{a}\n{b}"
            );
        }
        // A run of schemas, folded in order and (when long) by halves
        let len = [3, 5, 9, 12, 23][case % 5];
        let mut run = vec![a.clone(), b];
        while run.len() < len {
            let base = &run[rng.below(run.len() as u64) as usize];
            let next = mutate(&mut rng, base, 4);
            run.push(next);
        }
        let run: Vec<&Value> = run.iter().collect();
        let borrowed = borrowed_fold(&run, &config);
        cases += borrowed.is_some() as usize;
        assert_eq!(
            text(unify_field_schemas_sequential("f", &run, "p", &config)),
            text(borrowed),
            "{run:?}"
        );
        assert_eq!(
            text(unify_field_schemas_parallel("f", &run, "p", &config)),
            text(borrowed_halves(&run, &config)),
            "{run:?}"
        );
    }
    // Enough of them unify to compare more than failures
    assert!(unified > 1000 && cases > 500, "{unified} pairs and {cases} runs unified");
}
