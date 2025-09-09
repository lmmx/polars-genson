//! Convert JSON Schema to Polars types.

use crate::types::conversion_error;
use polars::prelude::*;
use serde_json::Value;

/// The type of schema to be deserialised to Polars schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormat {
    JsonSchema,
    Avro,
}

/// Convert JSON schema to Polars field mappings.
///
/// Returns a vector of (field_name, dtype_string) pairs that can be used
/// to construct Polars schemas.
///
/// * `format=SchemaFormat::JsonSchema` expects Draft-07 style JSON Schema.
/// * `format=SchemaFormat::Avro` expects Avro schema JSON (record at top-level).
pub fn schema_to_polars_fields(
    schema: &Value,
    format: SchemaFormat,
    debug: bool,
) -> Result<Vec<(String, String)>, PolarsError> {
    if debug {
        eprintln!("=== Generated Schema ({:?}) ===", format);
        eprintln!(
            "{}",
            serde_json::to_string_pretty(schema)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );
        eprintln!("==============================");
    }

    match format {
        SchemaFormat::JsonSchema => json_schema_to_polars_fields(schema),
        SchemaFormat::Avro => avro_schema_to_polars_fields(schema),
    }
}

/// Convert JSON Schema object to Polars field mappings.
fn json_schema_to_polars_fields(json_schema: &Value) -> Result<Vec<(String, String)>, PolarsError> {
    let mut fields = Vec::new();
    if let Some(properties) = json_schema.get("properties").and_then(|p| p.as_object()) {
        for (field_name, field_schema) in properties {
            let polars_type = json_type_to_polars_type(field_schema)?;
            fields.push((field_name.clone(), polars_type));
        }
    }
    Ok(fields)
}

/// Convert Avro record schema to Polars field mappings.
fn avro_schema_to_polars_fields(avro_schema: &Value) -> Result<Vec<(String, String)>, PolarsError> {
    let mut fields = Vec::new();
    if let Some(avro_fields) = avro_schema.get("fields").and_then(|f| f.as_array()) {
        for f in avro_fields {
            if let (Some(name), Some(field_type)) = (f.get("name"), f.get("type")) {
                let fname = name.as_str().unwrap_or("").to_string();
                let ftype = avro_type_to_polars_type(field_type)?;
                fields.push((fname, ftype));
            }
        }
    }
    Ok(fields)
}

/// Convert a JSON Schema type definition to Polars DataType string representation.
pub fn json_type_to_polars_type(json_schema: &Value) -> Result<String, PolarsError> {
    if let Some(type_value) = json_schema.get("type") {
        match type_value.as_str() {
            Some("string") => Ok("String".to_string()),
            Some("integer") => Ok("Int64".to_string()),
            Some("number") => Ok("Float64".to_string()),
            Some("boolean") => Ok("Boolean".to_string()),
            Some("null") => Ok("Null".to_string()),
            Some("array") => {
                // Handle arrays with item types
                if let Some(items) = json_schema.get("items") {
                    let item_type = json_type_to_polars_type(items)?;
                    Ok(format!("List[{}]", item_type))
                } else {
                    Ok("List".to_string()) // Fallback for arrays without item info
                }
            }
            Some("object") => {
                // Handle nested objects/structs
                if let Some(properties) = json_schema.get("properties").and_then(|p| p.as_object())
                {
                    let mut struct_fields = Vec::new();
                    for (field_name, field_schema) in properties {
                        let field_type = json_type_to_polars_type(field_schema)?;
                        struct_fields.push(format!("{}:{}", field_name, field_type));
                    }
                    Ok(format!("Struct[{}]", struct_fields.join(",")))
                } else {
                    Ok("Struct".to_string()) // Fallback for objects without properties
                }
            }
            Some(other) => Err(conversion_error(format!(
                "Unsupported JSON Schema type: {}",
                other
            ))),
            None => Ok("String".to_string()), // Default fallback
        }
    } else {
        Ok("String".to_string()) // Default fallback
    }
}

/// Convert an Avro type definition to Polars DataType string representation.
pub fn avro_type_to_polars_type(avro_schema: &Value) -> Result<String, PolarsError> {
    match avro_schema {
        // Primitive types
        Value::String(s) => match s.as_str() {
            "string" => Ok("String".to_string()),
            "int" | "long" => Ok("Int64".to_string()),
            "float" | "double" => Ok("Float64".to_string()),
            "boolean" => Ok("Boolean".to_string()),
            "null" => Ok("Null".to_string()),
            other => Err(conversion_error(format!(
                "Unsupported Avro type: {}",
                other
            ))),
        },

        // Array type
        Value::Object(obj) if obj.get("type") == Some(&Value::String("array".into())) => {
            if let Some(items) = obj.get("items") {
                let item_type = avro_type_to_polars_type(items)?;
                Ok(format!("List[{}]", item_type))
            } else {
                Ok("List".to_string()) // fallback if no items schema
            }
        }

        // Map type → represented as list of {key,value} structs in Polars
        Value::Object(obj) if obj.get("type") == Some(&Value::String("map".into())) => {
            if let Some(values) = obj.get("values") {
                let value_type = avro_type_to_polars_type(values)?;
                Ok(format!("List[Struct[key:String,value:{}]]", value_type))
            } else {
                Ok("List[Struct[key:String,value:String]]".to_string()) // fallback default
            }
        }

        // Record type → Polars Struct
        Value::Object(obj) if obj.get("type") == Some(&Value::String("record".into())) => {
            let mut struct_fields = Vec::new();
            if let Some(fields) = obj.get("fields").and_then(|f| f.as_array()) {
                for field in fields {
                    if let (Some(name), Some(ftype)) = (field.get("name"), field.get("type")) {
                        let fname = name.as_str().unwrap_or("").to_string();
                        let ftype_str = avro_type_to_polars_type(ftype)?;
                        struct_fields.push(format!("{}:{}", fname, ftype_str));
                    }
                }
            }
            Ok(format!("Struct[{}]", struct_fields.join(",")))
        }

        // Union type → pick first non-null branch
        Value::Array(types) => {
            let non_null = types.iter().find(|t| *t != &Value::String("null".into()));
            if let Some(branch) = non_null {
                avro_type_to_polars_type(branch)
            } else {
                Ok("Null".to_string())
            }
        }

        // Fallback
        _ => Err(conversion_error(format!(
            "Unsupported Avro schema element: {}",
            avro_schema
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_types() {
        assert_eq!(
            json_type_to_polars_type(&json!({"type": "string"})).unwrap(),
            "String"
        );
        assert_eq!(
            json_type_to_polars_type(&json!({"type": "integer"})).unwrap(),
            "Int64"
        );
        assert_eq!(
            json_type_to_polars_type(&json!({"type": "boolean"})).unwrap(),
            "Boolean"
        );
    }

    #[test]
    fn test_array_type() {
        let array_schema = json!({
            "type": "array",
            "items": {"type": "string"}
        });
        assert_eq!(
            json_type_to_polars_type(&array_schema).unwrap(),
            "List[String]"
        );
    }

    #[test]
    fn test_struct_type() {
        let struct_schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });
        let result = json_type_to_polars_type(&struct_schema).unwrap();
        // Note: order might vary due to HashMap iteration
        assert!(result.starts_with("Struct["));
        assert!(result.contains("name:String"));
        assert!(result.contains("age:Int64"));
    }
}
