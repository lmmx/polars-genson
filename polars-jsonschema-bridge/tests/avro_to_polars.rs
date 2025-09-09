/// Converted from README Block 2 - Avro Schema to Polars conversion with complex nested structure
#[test]
fn avro_schema_to_polars() -> eyre::Result<()> {
    use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
    use serde_json::json;

    let avro_schema = json!({
        "type": "record",
        "name": "document",
        "namespace": "demo",
        "fields": [
            {
                "name": "name",
                "type": ["null", "string"]
            },
            {
                "name": "age",
                "type": ["null", "int"]
            },
            {
                "name": "scores",
                "type": [
                    "null",
                    {
                        "type": "array",
                        "items": "float"
                    }
                ]
            },
            {
                "name": "profile",
                "type": [
                    "null",
                    {
                        "type": "record",
                        "name": "profile",
                        "namespace": "demo.document_types",
                        "fields": [
                            {
                                "name": "active",
                                "type": ["null", "boolean"]
                            }
                        ]
                    }
                ]
            }
        ]
    });

    let fields = schema_to_polars_fields(&avro_schema, SchemaFormat::Avro, false)?;

    // Convert to a more snapshot-friendly format
    let fields_vec: Vec<(String, String)> = fields.into_iter().collect();

    insta::assert_yaml_snapshot!("avro_to_polars", fields_vec);
    Ok(())
}

/// Avro Schema to Polars conversion with a map type (from polars-genson-py normalisation tests)
#[test]
fn avro_schema_with_map_demo() -> eyre::Result<()> {
    use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
    use serde_json::json;

    let avro_schema = json!({
        "type": "record",
        "name": "document",
        "namespace": "genson",
        "fields": [
            {
                "name": "id",
                "type": "int"
            },
            {
                "name": "tags",
                "type": [
                    "null",
                    {
                        "type": "array",
                        "items": "string"
                    }
                ]
            },
            {
                "name": "labels",
                "type": {
                    "name": "labels",
                    "type": "map",
                    "values": "string"
                }
            },
            {
                "name": "active",
                "type": [
                    "null",
                    "boolean"
                ]
            }
        ]
    });

    let fields = schema_to_polars_fields(&avro_schema, SchemaFormat::Avro, false)?;
    let fields_vec: Vec<(String, String)> = fields.into_iter().collect();

    insta::assert_yaml_snapshot!("avro_map_demo_to_polars", fields_vec);
    Ok(())
}
