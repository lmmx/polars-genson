#[cfg(not(panic = "unwind"))]
compile_error!("genson-core requires panic=unwind to catch genson-rs panics. Set [profile.*].panic = \"unwind\" in Cargo.toml.");

pub mod schema;

// Re-export commonly used items
pub use schema::{infer_json_schema_from_strings, SchemaInferenceConfig, SchemaInferenceResult};

/// Helper function to infer JSON schema from a collection of JSON strings
pub fn infer_json_schema(
    json_strings: &[String],
    config: Option<SchemaInferenceConfig>,
) -> Result<SchemaInferenceResult, String> {
    infer_json_schema_from_strings(json_strings, config.unwrap_or_default())
}

/// Create a default schema inference configuration
pub fn default_config() -> SchemaInferenceConfig {
    SchemaInferenceConfig::default()
}
