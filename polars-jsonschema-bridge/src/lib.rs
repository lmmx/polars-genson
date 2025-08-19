// src/lib.rs
use polars::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Unsupported Polars DataType: {0}")]
    UnsupportedPolarsType(String),
    #[error("Unsupported JSON Schema type: {0}")]
    UnsupportedJsonSchemaType(String),
    #[error("Invalid JSON Schema format: {0}")]
    InvalidJsonSchema(String),
    #[error("Polars error: {0}")]
    PolarsError(#[from] PolarsError),
}

pub type Result<T> = std::result::Result<T, ConversionError>;

/// Convert a Polars Schema to JSON Schema
pub fn polars_schema_to_json_schema(schema: &Schema) -> Result<Value> {
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

/// Convert a JSON Schema to Polars Schema
pub fn json_schema_to_polars_schema(json_schema: &Value) -> Result<Schema> {
    let properties = json_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .ok_or_else(|| {
            ConversionError::InvalidJsonSchema("Missing 'properties' field".to_string())
        })?;

    let _required_fields: Vec<&str> = json_schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut schema = Schema::default();

    for (field_name, field_schema) in properties {
        let dtype = json_schema_to_polars_dtype(field_schema)?;

        // For now, treat all fields as required if they're in the required array
        // In a more sophisticated implementation, you might handle nullable fields differently
        schema.with_column(field_name.clone().into(), dtype);
    }

    Ok(schema)
}

/// Convert a Polars DataType to JSON Schema type definition
pub fn polars_dtype_to_json_schema(dtype: &DataType) -> Result<Value> {
    match dtype {
        DataType::Boolean => Ok(json!({"type": "boolean"})),

        // Integer types
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 | DataType::Int128 => {
            Ok(json!({"type": "integer"}))
        }
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => Ok(json!({
            "type": "integer",
            "minimum": 0
        })),

        // Float types
        DataType::Float32 | DataType::Float64 => Ok(json!({"type": "number"})),

        // String types
        DataType::String => Ok(json!({"type": "string"})),

        // Date/Time types
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

        // Array types
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

        // Struct type
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

        // Binary types
        DataType::Binary | DataType::BinaryOffset => Ok(json!({
            "type": "string",
            "contentEncoding": "base64"
        })),

        // Decimal
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

        // Null
        DataType::Null => Ok(json!({"type": "null"})),

        // Unsupported types
        DataType::Categorical(_, _) | DataType::Enum(_, _) => {
            // For now, treat categorical/enum as string
            Ok(json!({
                "type": "string",
                "description": "Categorical data represented as string"
            }))
        }

        DataType::Object(_) | DataType::Unknown(_) => Err(ConversionError::UnsupportedPolarsType(
            format!("{:?}", dtype),
        )),
    }
}

/// Convert a JSON Schema type definition to Polars DataType
pub fn json_schema_to_polars_dtype(json_schema: &Value) -> Result<DataType> {
    let schema_type = json_schema
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| ConversionError::InvalidJsonSchema("Missing 'type' field".to_string()))?;

    match schema_type {
        "boolean" => Ok(DataType::Boolean),

        "integer" => {
            // Check if it's unsigned (has minimum: 0)
            if json_schema.get("minimum").and_then(|m| m.as_i64()) == Some(0) {
                Ok(DataType::UInt64) // Default to UInt64 for unsigned
            } else {
                Ok(DataType::Int64) // Default to Int64 for signed
            }
        }

        "number" => Ok(DataType::Float64),

        "string" => {
            // Check for format hints
            match json_schema.get("format").and_then(|f| f.as_str()) {
                Some("date") => Ok(DataType::Date),
                Some("date-time") => Ok(DataType::Datetime(TimeUnit::Microseconds, None)),
                Some("time") => Ok(DataType::Time),
                _ => {
                    // Check for binary encoding
                    if json_schema.get("contentEncoding").and_then(|e| e.as_str()) == Some("base64")
                    {
                        Ok(DataType::Binary)
                    } else {
                        Ok(DataType::String)
                    }
                }
            }
        }

        "array" => {
            let items_schema = json_schema.get("items").ok_or_else(|| {
                ConversionError::InvalidJsonSchema("Array missing 'items' field".to_string())
            })?;

            let inner_dtype = json_schema_to_polars_dtype(items_schema)?;

            // Check if it's a fixed-size array
            if let (Some(min_items), Some(max_items)) = (
                json_schema.get("minItems").and_then(|m| m.as_u64()),
                json_schema.get("maxItems").and_then(|m| m.as_u64()),
            ) {
                if min_items == max_items {
                    return Ok(DataType::Array(Box::new(inner_dtype), min_items as usize));
                }
            }

            Ok(DataType::List(Box::new(inner_dtype)))
        }

        "object" => {
            let properties = json_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .ok_or_else(|| {
                    ConversionError::InvalidJsonSchema(
                        "Object missing 'properties' field".to_string(),
                    )
                })?;

            let mut fields = Vec::new();

            for (field_name, field_schema) in properties {
                let field_dtype = json_schema_to_polars_dtype(field_schema)?;
                fields.push(Field::new(field_name.clone().into(), field_dtype));
            }

            Ok(DataType::Struct(fields))
        }

        "null" => Ok(DataType::Null),

        _ => Err(ConversionError::UnsupportedJsonSchemaType(
            schema_type.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types() {
        // Test boolean
        let bool_schema = polars_dtype_to_json_schema(&DataType::Boolean).unwrap();
        assert_eq!(bool_schema, json!({"type": "boolean"}));

        // Test string
        let string_schema = polars_dtype_to_json_schema(&DataType::String).unwrap();
        assert_eq!(string_schema, json!({"type": "string"}));

        // Test integer
        let int_schema = polars_dtype_to_json_schema(&DataType::Int64).unwrap();
        assert_eq!(int_schema, json!({"type": "integer"}));
    }

    #[test]
    fn test_list_type() {
        let list_dtype = DataType::List(Box::new(DataType::String));
        let list_schema = polars_dtype_to_json_schema(&list_dtype).unwrap();

        let expected = json!({
            "type": "array",
            "items": {"type": "string"}
        });

        assert_eq!(list_schema, expected);
    }

    #[test]
    fn test_round_trip() {
        // Create a simple schema
        let mut original_schema = Schema::default();
        original_schema.with_column("name".into(), DataType::String);
        original_schema.with_column("age".into(), DataType::Int64);
        original_schema.with_column("active".into(), DataType::Boolean);

        // Convert to JSON Schema and back
        let json_schema = polars_schema_to_json_schema(&original_schema).unwrap();
        let converted_schema = json_schema_to_polars_schema(&json_schema).unwrap();

        // Check that the essential structure is preserved
        assert_eq!(original_schema.len(), converted_schema.len());
        assert!(converted_schema.get("name").is_some());
        assert!(converted_schema.get("age").is_some());
        assert!(converted_schema.get("active").is_some());
    }
}
