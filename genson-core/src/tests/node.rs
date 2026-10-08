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
/// too), arrays (empty, of scalars, of objects, mixed; under 10 items, the most
/// `ListStrategy::add_object` merges serially), and every scalar type.
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
            let n = [0, 1, 2, 9][rng.below(4) as usize];
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

/// `absorb` leaves the same node as `add_schema` of the absorbed node's schema, whether the
/// nodes were built from data or from schemas.
#[test]
fn test_absorb_matches_adding_the_schema() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..5000 {
        let (target, other) = (random_documents(&mut rng), random_documents(&mut rng));
        let (target_from_schema, other_from_schema) = (rng.below(3) == 0, rng.below(3) == 0);

        let mut by_schema = build(&target, target_from_schema);
        let schema = build(&other, other_from_schema).to_schema();
        by_schema.add_schema(DataType::Schema(&schema));

        let mut absorbed = build(&target, target_from_schema);
        absorbed.absorb(build(&other, other_from_schema));

        assert_eq!(absorbed, by_schema, "{target:?} absorbing {other:?}");
    }
}
