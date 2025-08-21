//! Convert Polars types to JSON Schema.

use crate::types::conversion_error;
use polars::prelude::*;
use serde_json::{json, Map, Value};

/// Options for controlling JSON Schema generation
#[derive(Debug, Clone)]
pub struct JsonSchemaOptions {
    pub schema_uri: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub optional_fields: std::collections::HashSet<String>,
    pub additional_properties: bool,
}

impl Default for JsonSchemaOptions {
    fn default() -> Self {
        Self {
            schema_uri: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            title: None,
            description: None,
            optional_fields: std::collections::HashSet::new(),
            additional_properties: false,
        }
    }
}

impl JsonSchemaOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema_uri<S: Into<String>>(mut self, uri: Option<S>) -> Self {
        self.schema_uri = uri.map(|s| s.into());
        self
    }

    pub fn with_title<S: Into<String>>(mut self, title: Option<S>) -> Self {
        self.title = title.map(|s| s.into());
        self
    }

    pub fn with_description<S: Into<String>>(mut self, description: Option<S>) -> Self {
        self.description = description.map(|s| s.into());
        self
    }

    pub fn with_optional_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.optional_fields = fields.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn with_additional_properties(mut self, allow: bool) -> Self {
        self.additional_properties = allow;
        self
    }
}

/// Convert a Polars Schema to JSON Schema with preserved insertion order.
pub fn polars_schema_to_json_schema(
    schema: &Schema,
    options: &JsonSchemaOptions,
) -> Result<Value, PolarsError> {
    // Use serde_json::Map to preserve insertion order
    let mut properties = Map::new();
    let mut required = Vec::new();

    for (field_name, dtype) in schema.iter() {
        let field_schema = polars_dtype_to_json_schema(dtype, options)?;
        properties.insert(field_name.to_string(), field_schema);

        // Only add to required if not explicitly marked as optional
        if !options.optional_fields.contains(&field_name.to_string()) {
            required.push(field_name.to_string());
        }
    }

    let mut schema_obj = Map::new();

    // Add schema URI if provided
    if let Some(ref uri) = options.schema_uri {
        schema_obj.insert("$schema".to_string(), json!(uri));
    }

    // Add title and description if provided
    if let Some(ref title) = options.title {
        schema_obj.insert("title".to_string(), json!(title));
    }
    if let Some(ref description) = options.description {
        schema_obj.insert("description".to_string(), json!(description));
    }

    schema_obj.insert("type".to_string(), json!("object"));
    schema_obj.insert("properties".to_string(), json!(properties));

    if !required.is_empty() {
        required.sort();
        schema_obj.insert("required".to_string(), json!(required));
    }

    schema_obj.insert(
        "additionalProperties".to_string(),
        json!(options.additional_properties),
    );

    Ok(Value::Object(schema_obj))
}

/// Convert a Polars DataType to JSON Schema type definition.
pub fn polars_dtype_to_json_schema(
    dtype: &DataType,
    options: &JsonSchemaOptions,
) -> Result<Value, PolarsError> {
    match dtype {
        DataType::Boolean => Ok(json!({"type": "boolean"})),

        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => {
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

        DataType::Datetime(_time_unit, time_zone) => {
            let mut schema = json!({
                "type": "string",
                "format": "date-time"
            });

            if time_zone.is_some() {
                schema.as_object_mut().unwrap().insert(
                    "description".to_string(),
                    json!("Date-time with timezone information"),
                );
            }

            Ok(schema)
        }

        DataType::Time => Ok(json!({
            "type": "string",
            "format": "time"
        })),

        DataType::Duration(_) => Ok(json!({
            "type": "string",
            "format": "duration",
            "description": "ISO 8601 duration string"
        })),

        DataType::List(inner) => {
            let items_schema = polars_dtype_to_json_schema(inner, options)?;
            Ok(json!({
                "type": "array",
                "items": items_schema
            }))
        }

        DataType::Array(inner, size) => {
            let items_schema = polars_dtype_to_json_schema(inner, options)?;
            Ok(json!({
                "type": "array",
                "items": items_schema,
                "minItems": size,
                "maxItems": size
            }))
        }

        DataType::Struct(fields) => {
            let mut properties = Map::new();
            let mut required = Vec::new();

            for field in fields {
                let field_schema = polars_dtype_to_json_schema(field.dtype(), options)?;
                properties.insert(field.name().to_string(), field_schema);
                required.push(field.name().to_string());
            }

            Ok(json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": options.additional_properties
            }))
        }

        DataType::Binary => Ok(json!({
            "type": "string",
            "contentEncoding": "base64",
            "description": "Binary data encoded as base64"
        })),

        DataType::Decimal(precision, scale) => {
            let mut schema = json!({"type": "number"});

            if let (Some(p), Some(s)) = (precision, scale) {
                let obj = schema.as_object_mut().unwrap();
                obj.insert(
                    "description".to_string(),
                    json!(format!(
                        "Decimal number with precision {} and scale {}",
                        p, s
                    )),
                );

                // Add multipleOf for scale constraint
                if *s > 0 {
                    let multiple_of = 10_f64.powi(-(*s as i32));
                    obj.insert("multipleOf".to_string(), json!(multiple_of));
                }
            }

            Ok(schema)
        }

        DataType::Null => Ok(json!({"type": "null"})),

        DataType::Categorical(_, _ordering) => {
            let schema = json!({
                "type": "string",
                "description": "Categorical data"
            });

            // Could potentially add enum values if we had access to them
            Ok(schema)
        }

        DataType::Enum(_, _) => {
            // For enums, we treat them as string types
            // Note: Extracting specific enum values requires more complex handling
            Ok(json!({
                "type": "string",
                "description": "Enumerated string values"
            }))
        }

        // Handle newer Polars types
        DataType::Object(_) => Err(conversion_error(
            "Object types cannot be converted to JSON Schema".to_string(),
        )),

        DataType::Unknown(_) => Err(conversion_error(
            "Unknown types cannot be converted to JSON Schema".to_string(),
        )),

        // Fallback for any other types
        _ => Ok(json!({
            "type": "string",
            "description": format!("Unsupported Polars type: {:?}", dtype)
        })),
    }
}

/// Convert a DataFrame schema to JSON Schema - convenience function
pub fn dataframe_to_json_schema(
    df: &DataFrame,
    options: &JsonSchemaOptions,
) -> Result<Value, PolarsError> {
    polars_schema_to_json_schema(df.schema(), options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types() {
        let options = &JsonSchemaOptions::default();

        assert_eq!(
            polars_dtype_to_json_schema(&DataType::Boolean, options).unwrap(),
            json!({"type": "boolean"})
        );

        assert_eq!(
            polars_dtype_to_json_schema(&DataType::String, options).unwrap(),
            json!({"type": "string"})
        );

        assert_eq!(
            polars_dtype_to_json_schema(&DataType::Int64, options).unwrap(),
            json!({"type": "integer"})
        );
    }

    #[test]
    fn test_list_type() {
        let options = &JsonSchemaOptions::default();

        let list_dtype = DataType::List(Box::new(DataType::String));
        let result = polars_dtype_to_json_schema(&list_dtype, options).unwrap();

        let expected = json!({
            "type": "array",
            "items": {"type": "string"}
        });

        assert_eq!(result, expected);
    }

    #[test]
    fn test_struct_type() {
        let options = &JsonSchemaOptions::default();

        let fields = vec![
            Field::new("name".into(), DataType::String),
            Field::new("age".into(), DataType::Int64),
        ];
        let struct_dtype = DataType::Struct(fields);

        let result = polars_dtype_to_json_schema(&struct_dtype, options).unwrap();

        assert_eq!(result["type"], "object");
        assert!(result["properties"]["name"]["type"] == "string");
        assert!(result["properties"]["age"]["type"] == "integer");
    }

    #[test]
    fn test_schema_with_options() {
        let schema = Schema::from_iter(vec![
            Field::new("id".into(), DataType::Int64),
            Field::new("name".into(), DataType::String),
            Field::new("email".into(), DataType::String),
        ]);

        let options = JsonSchemaOptions::new()
            .with_title(Some("User Schema"))
            .with_optional_fields(vec!["email"]);

        let json_schema = polars_schema_to_json_schema(&schema, &options).unwrap();

        assert_eq!(json_schema["title"], "User Schema");
        assert_eq!(json_schema["type"], "object");

        let required: Vec<String> =
            serde_json::from_value(json_schema["required"].clone()).unwrap();

        assert!(required.contains(&"id".to_string()));
        assert!(required.contains(&"name".to_string()));
        assert!(!required.contains(&"email".to_string()));
    }

    #[test]
    fn test_decimal_type() {
        let options = &JsonSchemaOptions::default();

        let decimal_dtype = DataType::Decimal(Some(10), Some(2));
        let result = polars_dtype_to_json_schema(&decimal_dtype, options).unwrap();

        assert_eq!(result["type"], "number");
        assert_eq!(result["multipleOf"], 0.01);
        assert!(result["description"]
            .as_str()
            .unwrap()
            .contains("precision 10"));
    }

    #[test]
    fn test_categorical_type() {
        let options = &JsonSchemaOptions::default();

        // Test categorical type handling
        use polars::prelude::*;
        use std::sync::Arc;

        // Create a categorical type similar to your snapshot test
        let categories = Categories::new(
            PlSmallStr::from_static("test_cat"),
            PlSmallStr::from_static("test_namespace"),
            CategoricalPhysical::U8,
        );

        let categorical_dtype =
            DataType::Categorical(categories, Arc::new(CategoricalMapping::new(255)));

        let result = polars_dtype_to_json_schema(&categorical_dtype, options).unwrap();

        assert_eq!(result["type"], "string");
        assert_eq!(result["description"], "Categorical data");
    }

    #[test]
    fn test_required_fields_are_sorted() {
        let options = &JsonSchemaOptions::default();

        // Insert fields in an intentionally shuffled order
        let schema = Schema::from_iter(vec![
            Field::new("zeta".into(), DataType::Int64),
            Field::new("alpha".into(), DataType::String),
            Field::new("middle".into(), DataType::Boolean),
        ]);

        let json_schema = polars_schema_to_json_schema(&schema, options).unwrap();

        let required: Vec<String> =
            serde_json::from_value(json_schema["required"].clone()).unwrap();

        // Ensure the required list is alphabetically sorted
        assert_eq!(required, vec!["alpha", "middle", "zeta"]);
    }
}
