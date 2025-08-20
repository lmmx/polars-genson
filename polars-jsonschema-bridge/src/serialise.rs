//! Convert Polars types to JSON Schema.

use crate::types::conversion_error;
use polars::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Convert a Polars Schema to JSON Schema.
///
/// TODO: Column order is not preserved in Schema iteration.
/// Consider using IndexMap or storing column order separately
/// if order matters for your use case.
pub fn polars_schema_to_json_schema(schema: &Schema) -> Result<Value, PolarsError> {
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    for (field_name, dtype) in schema.iter() {
        properties.insert(field_name.as_str(), polars_dtype_to_json_schema(dtype)?);
        required.push(field_name.as_str());
    }

    Ok(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    }))
}

/// Convert a Polars DataType to JSON Schema type definition.
pub fn polars_dtype_to_json_schema(dtype: &DataType) -> Result<Value, PolarsError> {
    match dtype {
        DataType::Boolean => Ok(json!({"type": "boolean"})),

        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 | DataType::Int128 => {
            Ok(json!({"type": "integer"}))
        }

        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => Ok(json!({
            "type": "integer",
            "minimum": 0
        })),

        DataType::Float32 | DataType::Float64 => Ok(json!({"type": "number"})),

        DataType::String => Ok(json!({"type": "string"})),

        DataType::Date => Ok(json!({
            "type": "string",
            "format": "date"
        })),

        DataType::Datetime(_, _) => Ok(json!({
            "type": "string",
            "format": "date-time"
        })),

        DataType::Time => Ok(json!({
            "type": "string",
            "format": "time"
        })),

        DataType::Duration(_) => Ok(json!({
            "type": "string",
            "description": "ISO 8601 duration"
        })),

        DataType::List(inner) => {
            let items_schema = polars_dtype_to_json_schema(inner)?;
            Ok(json!({
                "type": "array",
                "items": items_schema
            }))
        }

        DataType::Array(inner, size) => {
            let items_schema = polars_dtype_to_json_schema(inner)?;
            Ok(json!({
                "type": "array",
                "items": items_schema,
                "minItems": size,
                "maxItems": size
            }))
        }

        DataType::Struct(fields) => {
            let mut properties = HashMap::new();
            let mut required = Vec::new();

            for field in fields {
                let field_schema = polars_dtype_to_json_schema(field.dtype())?;
                properties.insert(field.name().as_str(), field_schema);
                required.push(field.name().as_str());
            }

            Ok(json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            }))
        }

        DataType::Binary | DataType::BinaryOffset => Ok(json!({
            "type": "string",
            "contentEncoding": "base64"
        })),

        DataType::Decimal(precision, scale) => {
            let mut schema = json!({"type": "number"});
            if let (Some(p), Some(s)) = (precision, scale) {
                schema.as_object_mut().unwrap().insert(
                    "description".to_string(),
                    json!(format!("Decimal with precision {} and scale {}", p, s)),
                );
            }
            Ok(schema)
        }

        DataType::Null => Ok(json!({"type": "null"})),

        DataType::Categorical(_, _) | DataType::Enum(_, _) => Ok(json!({
            "type": "string",
            "description": "Categorical data represented as string"
        })),

        DataType::Object(_) | DataType::Unknown(_) => Err(conversion_error(format!(
            "Unsupported Polars DataType: {:?}",
            dtype
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types() {
        assert_eq!(
            polars_dtype_to_json_schema(&DataType::Boolean).unwrap(),
            json!({"type": "boolean"})
        );

        assert_eq!(
            polars_dtype_to_json_schema(&DataType::String).unwrap(),
            json!({"type": "string"})
        );

        assert_eq!(
            polars_dtype_to_json_schema(&DataType::Int64).unwrap(),
            json!({"type": "integer"})
        );
    }

    #[test]
    fn test_list_type() {
        let list_dtype = DataType::List(Box::new(DataType::String));
        let result = polars_dtype_to_json_schema(&list_dtype).unwrap();

        let expected = json!({
            "type": "array",
            "items": {"type": "string"}
        });

        assert_eq!(result, expected);
    }

    #[test]
    fn test_basic_schema_conversion() {
        let mut schema = Schema::default();
        schema.with_column("name".into(), DataType::String);
        schema.with_column("age".into(), DataType::Int64);

        let json_schema = polars_schema_to_json_schema(&schema).unwrap();

        assert!(json_schema.get("properties").is_some());
        assert!(json_schema.get("required").is_some());
        assert_eq!(json_schema.get("type").unwrap(), "object");
    }
}
