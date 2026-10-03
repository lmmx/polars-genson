use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::{json, Value};

/// Wikidata's claims options: every object a map candidate (map_threshold 0), compatible
/// records unified, and `datavalue` always promoted to a record of its own.
fn config() -> SchemaInferenceConfig {
    SchemaInferenceConfig {
        map_threshold: 0,
        unify_maps: true,
        force_scalar_promotion: ["datavalue".to_string()].into(),
        ..SchemaInferenceConfig::default()
    }
}

fn qualifier_snak(rows: &[&str]) -> Value {
    let schema = infer_json_schema_from_strings(rows, config())
        .unwrap()
        .schema;
    let q = &schema["properties"]["q"];
    assert!(q.get("properties").is_none(), "q should be a map: {q}");
    q["additionalProperties"]["items"].clone()
}

/// An object with a field in force_scalar_promotion is a record (the field is promoted
/// within it), even when every field is a string, which would otherwise make it a map
/// of strings: a qualifier snak whose datavalue is a string.
#[test]
fn test_object_with_promoted_field_stays_record() {
    let snak = qualifier_snak(&[
        r#"{"q": {"P1545": [{"snaktype": "value", "property": "P1545", "datavalue": "1", "datatype": "string"}]}}"#,
        r#"{"q": {"P1545": [{"snaktype": "value", "property": "P1545", "datavalue": "2", "datatype": "string"}]}}"#,
    ]);
    assert_eq!(snak["type"], json!("object"), "{snak}");
    assert!(snak.get("additionalProperties").is_none(), "{snak}");
    let props = snak["properties"].as_object().unwrap();
    assert_eq!(props["property"]["type"], json!("string"), "{snak}");
    assert!(
        props["datavalue"]["properties"]
            .get("datavalue__string")
            .is_some(),
        "datavalue should be promoted: {snak}"
    );
}

/// The same snak with an object datavalue in another row is the same record.
#[test]
fn test_object_with_promoted_field_mixed_rows() {
    let snak = qualifier_snak(&[
        r#"{"q": {"P1545": [{"snaktype": "value", "property": "P1545", "datavalue": "1", "datatype": "string"}]}}"#,
        r#"{"q": {"P642": [{"snaktype": "value", "property": "P642", "datavalue": {"id": "Q1"}, "datatype": "wikibase-item"}]}}"#,
    ]);
    assert_eq!(snak["type"], json!("object"), "{snak}");
    let dv = &snak["properties"]["datavalue"]["properties"];
    assert!(
        dv.get("datavalue__string").is_some() && dv.get("id").is_some(),
        "{snak}"
    );
}
