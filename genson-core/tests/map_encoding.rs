#![cfg(feature = "avro")]
use genson_core::normalise::{normalise_value, MapEncoding, NormaliseConfig};
use serde_json::json;

/// MapEncoding::Mapping: stays as a single object.
#[test]
fn test_map_encoding_mapping() {
    let schema = json!({"type":"map","values":"string"});
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Mapping,
        ..NormaliseConfig::default()
    };

    let input = json!({"en": "Hello", "fr": "Bonjour"});
    let norm = normalise_value(input, &schema, &cfg, None);

    assert_eq!(norm, json!({"en": "Hello", "fr": "Bonjour"}));
}

/// MapEncoding::Entries: becomes a list of single-entry objects.
#[test]
fn test_map_encoding_entries() {
    let schema = json!({"type":"map","values":"string"});
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Entries,
        ..NormaliseConfig::default()
    };

    let input = json!({"en": "Hello", "fr": "Bonjour"});
    let norm = normalise_value(input, &schema, &cfg, None);

    assert_eq!(
        norm,
        json!([
            {"en": "Hello"},
            {"fr": "Bonjour"}
        ])
    );
}

/// MapEncoding::KeyValueEntries: becomes a list of {key,value} objects.
#[test]
fn test_map_encoding_key_value_entries() {
    let schema = json!({"type":"map","values":"string"});
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        ..NormaliseConfig::default()
    };

    let input = json!({"en": "Hello", "fr": "Bonjour"});
    let norm = normalise_value(input, &schema, &cfg, None);

    assert_eq!(
        norm,
        json!([
            {"key": "en", "value": "Hello"},
            {"key": "fr", "value": "Bonjour"}
        ])
    );
}

/// Scalar fallback: ensure consistent encoding across modes.
#[test]
fn test_map_encoding_scalar_fallback() {
    let schema = json!({"type":"map","values":"string"});

    // Mapping
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Mapping,
        ..NormaliseConfig::default()
    };
    assert_eq!(
        normalise_value(json!("foo"), &schema, &cfg, None),
        json!({"default": "foo"})
    );

    // Entries
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::Entries,
        ..NormaliseConfig::default()
    };
    assert_eq!(
        normalise_value(json!("foo"), &schema, &cfg, None),
        json!([{"default": "foo"}])
    );

    // KeyValueEntries
    let cfg = NormaliseConfig {
        map_encoding: MapEncoding::KeyValueEntries,
        ..NormaliseConfig::default()
    };
    assert_eq!(
        normalise_value(json!("foo"), &schema, &cfg, None),
        json!([{"key": "default", "value": "foo"}])
    );
}
