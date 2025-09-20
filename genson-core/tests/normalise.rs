#![cfg(feature = "avro")]

use genson_core::normalise::{normalise_value, normalise_values, MapEncoding, NormaliseConfig};
use serde_json::json;

/// Arrays: empty → null (with flag), empty → [] (without flag).
#[test]
fn test_array_empty_behavior() {
    let schema = json!({"type":"array","items":"string"});

    // empty_as_null = true
    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };
    assert_eq!(normalise_value(json!([]), &schema, &cfg, None), json!(null));

    // empty_as_null = false
    let cfg = NormaliseConfig {
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    assert_eq!(normalise_value(json!([]), &schema, &cfg, None), json!([]));

    // scalar coerced into array
    assert_eq!(
        normalise_value(json!("foo"), &schema, &cfg, None),
        json!(["foo"])
    );
}

/// Maps: empty → null (with flag), empty → {} (without flag).
#[test]
fn test_map_empty_behavior() {
    let schema = json!({"type":"map","values":"string"});

    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };
    assert_eq!(normalise_value(json!({}), &schema, &cfg, None), json!(null));

    let cfg = NormaliseConfig {
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    assert_eq!(normalise_value(json!({}), &schema, &cfg, None), json!({}));

    // Fallback scalar coerced into map
    let val = normalise_value(json!("foo"), &schema, &cfg, None);
    assert_eq!(val, json!({"default":"foo"}));
}

/// Nested record with array field, ensures row is preserved.
#[test]
fn test_nested_record_array_field() {
    let schema = json!({
        "type":"record",
        "name":"doc",
        "fields":[
            {"name":"id","type":"string"},
            {"name":"tags","type":{"type":"array","items":"string"}}
        ]
    });

    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };
    let input = json!({"id":"1","tags":[]});
    let norm = normalise_value(input, &schema, &cfg, None);
    assert_eq!(norm, json!({"id":"1","tags":null}));

    let cfg = NormaliseConfig {
        empty_as_null: false,
        ..NormaliseConfig::default()
    };
    let input = json!({"id":"1","tags":[]});
    let norm = normalise_value(input, &schema, &cfg, None);
    assert_eq!(norm, json!({"id":"1","tags":[]}));
}

/// Union precedence: ["null","array"] should pick array branch if not null.
#[test]
fn test_union_precedence_array() {
    let schema = json!(["null", {"type":"array","items":"string"}]);
    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };

    // null stays null
    assert_eq!(
        normalise_value(json!(null), &schema, &cfg, None),
        json!(null)
    );

    // scalar coerced to array
    let val = normalise_value(json!("x"), &schema, &cfg, None);
    assert_eq!(val, json!(["x"]));
}

/// Union precedence: ["null","map","string"] should pick map branch.
#[test]
fn test_union_precedence_map() {
    let schema = json!(["null", {"type":"map","values":"string"}, "string"]);
    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };

    // scalar coerced into map, because map branch is first non-null
    let val = normalise_value(json!("foo"), &schema, &cfg, None);
    assert_eq!(val, json!({"default":"foo"}));
}

/// End-to-end: vector of values stays same length (no row loss).
#[test]
fn test_normalise_values_preserves_length() {
    let schema = json!({"type":"array","items":"string"});

    let cfg = NormaliseConfig {
        empty_as_null: true,
        ..NormaliseConfig::default()
    };
    let inputs = vec![
        json!(["a", "b"]),
        json!([]), // becomes null, but row stays
        json!(null),
    ];
    let outputs = normalise_values(inputs.clone(), &schema, &cfg);

    assert_eq!(outputs.len(), inputs.len());
    assert_eq!(outputs[0], json!(["a", "b"]));
    assert_eq!(outputs[1], json!(null));
    assert_eq!(outputs[2], json!(null));
}

/// Maps: scalar fallback respects map encoding.
#[test]
fn test_map_scalar_fallback_encodings() {
    let schema = json!({"type":"map","values":"string"});

    // Default Mapping
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Mapping,
        ..NormaliseConfig::default()
    };
    let val = normalise_value(json!("foo"), &schema, &cfg, None);
    assert_eq!(val, json!({"default": "foo"}));

    // Entries
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Entries,
        ..NormaliseConfig::default()
    };
    let val = normalise_value(json!("foo"), &schema, &cfg, None);
    assert_eq!(val, json!([{"default": "foo"}]));

    // KeyValueEntries
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        ..NormaliseConfig::default()
    };
    let val = normalise_value(json!("foo"), &schema, &cfg, None);
    assert_eq!(val, json!([{"key": "default", "value": "foo"}]));
}
