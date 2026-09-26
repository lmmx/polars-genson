use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};

/// Keys z..a then Z..A: 52 keys, in an order that is neither sorted nor hash order.
fn keys() -> Vec<String> {
    ('a'..='z')
        .rev()
        .chain(('A'..='Z').rev())
        .map(String::from)
        .collect()
}

fn properties(json: &str) -> Vec<String> {
    let schema = infer_json_schema_from_strings(&[json], SchemaInferenceConfig::default())
        .unwrap()
        .schema;
    schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

/// An object's properties keep the order the keys appear in, however many keys there are.
#[test]
fn test_wide_object_keeps_key_order() {
    let keys = keys();
    let row = format!(
        "{{{}}}",
        keys.iter()
            .enumerate()
            .map(|(i, k)| format!("\"{k}\": {i}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert_eq!(properties(&row), keys);
}
