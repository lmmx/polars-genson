//! Convert JSON Schema to Polars types.

use crate::types::conversion_error;
use polars::prelude::*;
use serde_json::Value;

/// Convert JSON schema to Polars field mappings.
///
/// Returns a vector of (field_name, dtype_string) pairs that can be used
/// to construct Polars schemas.
pub fn json_schema_to_polars_fields(
    json_schema: &Value,
    debug: bool,
) -> Result<Vec<(String, String)>, PolarsError> {
    let mut fields = Vec::new();

    if debug {
        eprintln!("=== Generated JSON Schema ===");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(json_schema)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        );
        eprintln!("=============================");
    }

    if let Some(properties) = json_schema.get("properties").and_then(|p| p.as_object()) {
        for (field_name, field_schema) in properties {
            let polars_type = json_type_to_polars_type(field_schema)?;
            fields.push((field_name.clone(), polars_type));
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

/// Convert JSON schema to a full Polars Schema.
///
/// Note: This function currently returns an error as it requires implementing
/// string → DataType parsing. Use `json_schema_to_polars_fields` for now.
pub fn json_schema_to_polars_schema(_json_schema: &Value) -> Result<Schema, PolarsError> {
    // TODO: Implement conversion from dtype strings to actual DataTypes
    // This would require parsing strings like "List[String]" back to DataType::List(Box::new(DataType::String))
    Err(conversion_error(
        "Full schema conversion not yet implemented",
    ))
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
