// genson-core/src/tests/node.rs
use super::*;

/// A small deterministic generator (xorshift), so the documents are the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random JSON document over a few keys, so that documents overlap: objects (empty ones
/// too), arrays (empty, of scalars, of objects, mixed; some of 10 items or more, which
/// `ListStrategy::add_object` merges in parallel), and every scalar type.
fn random_json(rng: &mut Rng, depth: u32) -> String {
    let pick = if depth == 0 { 4 + rng.below(5) } else { rng.below(9) };
    match pick {
        0 | 1 => {
            let n = rng.below(4);
            let fields: Vec<String> = (0..n)
                .map(|_| {
                    let key = ["a", "b", "c", "d"][rng.below(4) as usize];
                    format!("\"{key}\": {}", random_json(rng, depth - 1))
                })
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        2 | 3 => {
            let n = [0, 1, 2, 12][rng.below(4) as usize];
            let items: Vec<String> = (0..n).map(|_| random_json(rng, depth - 1)).collect();
            format!("[{}]", items.join(", "))
        }
        4 => "null".into(),
        5 => "true".into(),
        6 => rng.below(100).to_string(),
        7 => "1.5".into(),
        _ => "\"s\"".into(),
    }
}

fn node_of(documents: &[String]) -> SchemaNode {
    let mut node = SchemaNode::new();
    for document in documents {
        let mut bytes = document.clone().into_bytes();
        let tape = simd_json::to_tape(&mut bytes).unwrap();
        node.add_object(DataType::Object(tape.as_value()));
    }
    node
}

fn random_documents(rng: &mut Rng) -> Vec<String> {
    (0..rng.below(4)).map(|_| random_json(rng, 3)).collect()
}

/// The node built from `documents`, or else the node built by adding that one's schema to a
/// new node, which holds the placeholders of the schema's keywords.
fn build(documents: &[String], from_schema: bool) -> SchemaNode {
    let node = node_of(documents);
    if !from_schema {
        return node;
    }
    let mut by_schema = SchemaNode::new();
    by_schema.add_schema(DataType::Schema(&node.to_schema()));
    by_schema
}

/// `absorb` leaves the same node as `add_schema` of the absorbed node's schema, for a node
/// built from a schema (which holds no `required` set its schema leaves out).
#[test]
fn test_absorb_matches_adding_the_schema() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..5000 {
        let (target, other) = (random_documents(&mut rng), random_documents(&mut rng));
        let target_from_schema = rng.below(3) == 0;

        let mut by_schema = build(&target, target_from_schema);
        by_schema.add_schema(DataType::Schema(&build(&other, true).to_schema()));

        let mut absorbed = build(&target, target_from_schema);
        absorbed.absorb(build(&other, true));

        assert_eq!(absorbed, by_schema, "{target:?} absorbing {other:?}");
    }
}

/// Absorbing the node built from some documents gives the schema that building from all the
/// documents does (up to the order of keywords), `required` included: a key is required
/// only when every object seen holds it.
#[test]
fn test_absorb_matches_adding_the_documents() {
    let mut rng = Rng(0x5851_f42d_4c95_7f2d);
    for _ in 0..5000 {
        let (target, other) = (random_documents(&mut rng), random_documents(&mut rng));
        let mut absorbed = node_of(&target);
        absorbed.absorb(node_of(&other));
        let together = node_of(&[target.clone(), other.clone()].concat());
        assert_eq!(
            absorbed.to_schema(),
            together.to_schema(),
            "{target:?} absorbing {other:?}"
        );
    }
}

/// An object with none of a key, seen in one merged node, leaves that key not required.
#[test]
fn test_absorb_keeps_a_key_missing_from_an_empty_object_not_required() {
    for (first, second) in [("{\"a\": 1}", "{}"), ("{}", "{\"a\": 1}")] {
        let mut absorbed = node_of(&[first.to_string()]);
        absorbed.absorb(node_of(&[second.to_string()]));
        assert_eq!(absorbed.to_schema().get("required"), None, "{first} then {second}");
    }
}

/// `absorb_all` leaves the same node as `absorb` of each node in turn, with enough nodes
/// to take the parallel path, and documents that are mostly objects so that it is taken.
#[test]
fn test_absorb_all_matches_absorbing_in_turn() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..300 {
        let n = PAR_MERGE_MIN + rng.below(8) as usize;
        let rows: Vec<Vec<String>> = (0..n)
            .map(|_| {
                let fields: Vec<String> = (0..rng.below(4))
                    .map(|_| {
                        let key = ["a", "b", "c"][rng.below(3) as usize];
                        format!("\"{key}\": {}", random_json(&mut rng, 2))
                    })
                    .collect();
                vec![format!("{{{}}}", fields.join(", "))]
            })
            .collect();
        let from_schema = rng.below(3) == 0;
        let target: Vec<String> = random_documents(&mut rng);

        let mut in_turn = build(&target, from_schema);
        for row in &rows {
            in_turn.absorb(node_of(row));
        }
        let mut all = build(&target, from_schema);
        all.absorb_all(rows.iter().map(|row| node_of(row)).collect());
        assert_eq!(all, in_turn, "{target:?} absorbing {rows:?}");

        let mut from_empty = SchemaNode::new();
        from_empty.absorb_all(rows.iter().map(|row| node_of(row)).collect());
        let mut in_turn = SchemaNode::new();
        rows.iter().for_each(|row| in_turn.absorb(node_of(row)));
        assert_eq!(from_empty, in_turn, "absorbing {rows:?}");
    }
}

/// Two nodes hash equal exactly when their schemas are equal.
#[test]
fn test_hash_schema_matches_schema_equality() {
    let hash = |node: &SchemaNode| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        node.hash_schema(&mut hasher);
        std::hash::Hasher::finish(&hasher)
    };
    let mut rng = Rng(0xfeed_beef_dead_cafe);
    let mut equal = 0;
    for _ in 0..20000 {
        let (a, b) = (random_documents(&mut rng), random_documents(&mut rng));
        let (a, b) = (build(&a, rng.below(3) == 0), build(&b, rng.below(3) == 0));
        // Compared as text: `Value` equality ignores the order of an object's keys
        let text = |node: &SchemaNode| serde_json::to_string(&node.to_schema()).unwrap();
        let same_schema = text(&a) == text(&b);
        assert_eq!(hash(&a) == hash(&b), same_schema, "{} vs {}", a.to_schema(), b.to_schema());
        equal += same_schema as usize;
    }
    assert!(equal > 100, "too few equal pairs ({equal}) to test anything");
}

/// A hasher that keeps the bytes it is fed.
#[derive(Default)]
struct Recorder(Vec<u8>);

impl Hasher for Recorder {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn finish(&self) -> u64 {
        0
    }
}

/// A scalar strategy hashes as `S` and the compact serialisation of its schema, with
/// keywords or without.
#[test]
fn test_scalar_hash_is_its_serialised_schema() {
    let schemas = [
        json!({"type": "null"}),
        json!({"type": "boolean"}),
        json!({"type": "integer"}),
        json!({"type": "number"}),
        json!({"type": "string"}),
        json!({"type": "string", "format": "date"}),
        json!({"type": "integer", "minimum": 0}),
    ];
    for schema in schemas {
        let mut node = SchemaNode::new();
        node.add_schema(DataType::Schema(&schema));
        assert_eq!(node.active_strategies.len(), 1, "{schema}");
        let strategy = &node.active_strategies[0];
        let mut written = Recorder::default();
        strategy.hash_schema(&mut written);
        let mut expected = vec![b'S'];
        serde_json::to_writer(&mut expected, &strategy.to_schema()).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&written.0),
            String::from_utf8_lossy(&expected),
            "{schema}"
        );
    }
}

/// A scalar with keywords hashes apart from the bare scalar and from one with other
/// keywords, and equal to one with the same keywords, alone or as a property.
#[test]
fn test_hash_schema_tells_keyworded_scalars_apart() {
    let hash = |schema: Value| {
        let mut node = SchemaNode::new();
        node.add_schema(DataType::Schema(&schema));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        node.hash_schema(&mut hasher);
        hasher.finish()
    };
    let in_object = |a: Value| json!({"type": "object", "properties": {"a": a}});
    let bare = json!({"type": "string"});
    let date = json!({"type": "string", "format": "date"});
    let time = json!({"type": "string", "format": "time"});
    for wrap in [|schema| schema, in_object] {
        assert_ne!(hash(wrap(bare.clone())), hash(wrap(date.clone())));
        assert_ne!(hash(wrap(date.clone())), hash(wrap(time.clone())));
        assert_eq!(hash(wrap(date.clone())), hash(wrap(date.clone())));
    }
    assert_ne!(
        hash(json!({"type": "integer"})),
        hash(json!({"type": "number"}))
    );
}

/// `hash_schema` feeds the bytes it fed at ac5129c, so `SchemaBuilder::schema_hash` keeps
/// its values: one xxh64 over the nodes of 2000 random batches, a fifth of them with a
/// schema of keyworded scalars added, and the builder's hash of one document.
#[test]
fn test_hash_schema_values_are_unchanged() {
    let keyworded = json!({"type": "object", "properties": {
        "a": {"type": "string", "format": "date"},
        "b": {"type": "integer", "minimum": 0},
    }});
    let mut rng = Rng(0x0dd_ba11_c0ff_ee00);
    let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
    for i in 0..2000 {
        let mut node = build(&random_documents(&mut rng), rng.below(3) == 0);
        if i % 5 == 0 {
            node.add_schema(DataType::Schema(&keyworded));
        }
        node.hash_schema(&mut hasher);
    }
    assert_eq!(hasher.finish(), PINNED_NODES_HASH);

    let mut builder = crate::genson_rs::SchemaBuilder::new(None);
    let mut document = br#"{"a": 1, "b": [true, null, 1.5, "s"], "c": {"d": {}}}"#.to_vec();
    builder.add_object(simd_json::to_tape(&mut document).unwrap().as_value());
    assert_eq!(builder.schema_hash(), PINNED_BUILDER_HASH);
}

const PINNED_NODES_HASH: u64 = 4095574756938490984;
const PINNED_BUILDER_HASH: u64 = 11897637414506759766;

/// `add_schemas` into a new node leaves the same schema as `add_schema` of each schema in
/// turn.
#[test]
fn test_add_schemas_matches_adding_in_turn() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for _ in 0..3000 {
        let schemas: Vec<Value> = (0..1 + rng.below(5))
            .map(|_| build(&random_documents(&mut rng), false).to_schema())
            .collect();
        let schemas: Vec<&Value> = schemas.iter().collect();
        let mut in_turn = SchemaNode::new();
        for schema in &schemas {
            in_turn.add_schema(DataType::Schema(schema));
        }
        let mut batched = SchemaNode::new();
        batched.add_schemas(&schemas);
        assert_eq!(batched.to_schema(), in_turn.to_schema(), "adding {schemas:?}");
    }
}

/// `add_schemas` into a node already holding a schema leaves the same schema as
/// `add_schema` of each schema in turn into that node.
#[test]
fn test_add_schemas_into_a_seeded_node_matches_adding_in_turn() {
    let mut rng = Rng(0x6c07_8965_4f35_a2b1);
    for _ in 0..3000 {
        let seed = build(&random_documents(&mut rng), false).to_schema();
        let schemas: Vec<Value> = (0..1 + rng.below(5))
            .map(|_| build(&random_documents(&mut rng), false).to_schema())
            .collect();
        let schemas: Vec<&Value> = schemas.iter().collect();
        let mut in_turn = SchemaNode::new();
        in_turn.add_schema(DataType::Schema(&seed));
        for schema in &schemas {
            in_turn.add_schema(DataType::Schema(schema));
        }
        let mut batched = SchemaNode::new();
        batched.add_schema(DataType::Schema(&seed));
        batched.add_schemas(&schemas);
        assert_eq!(
            batched.to_schema(),
            in_turn.to_schema(),
            "seed {seed} adding {schemas:?}"
        );
    }
}

/// A property the seed and the batch share merges its shapes rather than taking the
/// batch's, whether the batch has few properties (merged in turn) or more than 3 (merged
/// in parallel).
#[test]
fn test_add_schemas_merges_a_property_the_node_already_holds() {
    for keys in [&["a", "b"][..], &["a", "b", "c", "d", "e"]] {
        let object = |value_type: &str| {
            let properties: serde_json::Map<String, Value> = keys
                .iter()
                .map(|key| (key.to_string(), json!({"type": value_type})))
                .collect();
            json!({"type": "object", "properties": properties, "required": keys})
        };
        let mut node = SchemaNode::new();
        node.add_schema(DataType::Schema(&object("integer")));
        node.add_schemas(&[&object("string")]);
        let schema = node.to_schema();
        for key in keys {
            assert_eq!(
                schema["properties"][key]["type"],
                json!(["integer", "string"]),
                "{} keys: {schema}",
                keys.len()
            );
        }
    }
}

/// A scalar node absorbing another of its kind leaves what `add_schema` of the other's
/// schema does: bare scalars of every type both ways round, and targets with keywords.
#[test]
fn test_absorb_scalar_matches_adding_its_schema() {
    let scalars = ["null", "true", "1", "1.5", "\"s\""];
    let schemas = [
        json!({"type": "string", "format": "date"}),
        json!({"type": "integer", "minimum": 0}),
        json!({"type": "number", "minimum": 0}),
    ];
    let target = |i: usize| match scalars.get(i) {
        Some(s) => node_of(&[s.to_string()]),
        None => {
            let mut node = SchemaNode::new();
            node.add_schema(DataType::Schema(&schemas[i - scalars.len()]));
            node
        }
    };
    for i in 0..scalars.len() + schemas.len() {
        for other in scalars {
            let other = || node_of(&[other.to_string()]);
            let mut by_schema = target(i);
            by_schema.add_schema(DataType::Schema(&other().to_schema()));
            let mut absorbed = target(i);
            absorbed.absorb(other());
            assert_eq!(
                absorbed,
                by_schema,
                "{} absorbing {}",
                target(i).to_schema(),
                other().to_schema()
            );
        }
    }
}
