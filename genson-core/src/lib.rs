#[cfg(not(panic = "unwind"))]
compile_error!("genson-core requires panic=unwind to catch genson-rs panics. Set [profile.*].panic = \"unwind\" in Cargo.toml.");

pub mod genson_rs;
#[cfg(feature = "avro")]
pub mod normalise;
pub mod schema;

// Re-export commonly used items
pub use schema::{
    infer_json_schema_from_strings, DebugVerbosity, SchemaInferenceConfig, SchemaInferenceResult,
};

/// Helper function to infer JSON schema from a collection of JSON strings
pub fn infer_json_schema(
    json_strings: &[String],
    config: Option<SchemaInferenceConfig>,
) -> Result<SchemaInferenceResult, String> {
    #[cfg(feature = "trace")]
    {
        use crustrace_mermaid::{GroupingMode, MermaidLayer};
        use tracing_subscriber::filter::LevelFilter;
        use tracing_subscriber::prelude::*;

        let mmd_layer = MermaidLayer::new()
            .with_mode(GroupingMode::MergeByName)
            .with_params_mode(crustrace_mermaid::ParamRenderMode::SingleNodeGrouped);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_span_events(
                        tracing_subscriber::fmt::format::FmtSpan::ENTER
                            | tracing_subscriber::fmt::format::FmtSpan::EXIT,
                    )
                    .with_filter(LevelFilter::INFO),
            )
            .with(mmd_layer)
            .init();
    }

    infer_json_schema_from_strings(json_strings, config.unwrap_or_default())
}

/// Create a default schema inference configuration
pub fn default_config() -> SchemaInferenceConfig {
    SchemaInferenceConfig::default()
}
