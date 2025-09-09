//! Bridge between Polars DataTypes and JSON Schema.
//!
//! Provides bidirectional conversion:
//! - JSON Schema → Polars types (deserialise)  
//! - Polars types → JSON Schema (serialise)
//!
//! # Examples
//!
//! ```rust
//! use polars_jsonschema_bridge::{schema_to_polars_fields, polars_schema_to_json_schema, JsonSchemaOptions};
//! use polars::prelude::*;
//! use serde_json::json;
//!
//! // JSON Schema → Polars
//! let json_schema = json!({
//!     "type": "object",
//!     "properties": {
//!         "name": {"type": "string"},
//!         "age": {"type": "integer"}
//!     }
//! });
//! let fields = schema_to_polars_fields(&json_schema, false).unwrap();
//!
//! // Polars → JSON Schema  
//! let mut schema = Schema::default();
//! schema.with_column("name".into(), DataType::String);
//! let json_schema = polars_schema_to_json_schema(&schema, &JsonSchemaOptions::new()).unwrap();
//! ```

pub mod deserialise;
pub mod serialise;
pub mod types;

// Re-export main functions
pub use deserialise::{json_type_to_polars_type, schema_to_polars_fields, SchemaFormat};
pub use serialise::{polars_dtype_to_json_schema, polars_schema_to_json_schema, JsonSchemaOptions};
pub use types::conversion_error;
