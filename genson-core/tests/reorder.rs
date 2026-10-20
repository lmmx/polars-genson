use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

#[test]
fn test_union_reordering_gives_stable_schema() {
    // Case A: null -> string -> int
    let jsons_a = vec![
        json!({"field": null}).to_string(),
        json!({"field": "hello"}).to_string(),
        json!({"field": 42}).to_string(),
    ];

    // Case B: int -> string -> null
    let jsons_b = vec![
        json!({"field": 42}).to_string(),
        json!({"field": "hello"}).to_string(),
        json!({"field": null}).to_string(),
    ];

    // By default a string and an integer each keep a field of their own; without
    // `wrap_scalars` the field stays their union
    for wrap_scalars in [true, false] {
        let config = SchemaInferenceConfig {
            wrap_scalars,
            ..SchemaInferenceConfig::default()
        };

        let schema_a = infer_json_schema_from_strings(&jsons_a, config.clone())
            .expect("Schema A should succeed")
            .schema;

        let schema_b = infer_json_schema_from_strings(&jsons_b, config)
            .expect("Schema B should succeed")
            .schema;

        // They should be equal once unions are normalised
        assert_eq!(
            schema_a, schema_b,
            "Schemas should be stable regardless of input order"
        );

        let field = &schema_a["properties"]["field"];
        if wrap_scalars {
            // The promoted fields in canonical order
            let promoted: Vec<&String> = field["properties"].as_object().unwrap().keys().collect();
            assert_eq!(promoted, ["field__integer", "field__string"]);
        } else {
            // And the union type should be in canonical order
            assert_eq!(
                field["type"],
                json!(["null", "integer", "string"]),
                "Union should be rewritten into stable precedence order"
            );
        }
    }
}
