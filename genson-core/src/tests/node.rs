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
