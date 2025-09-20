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
