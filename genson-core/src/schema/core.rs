use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInferenceConfig {
    /// Whether to treat top-level arrays as streams of objects
    pub ignore_outer_array: bool,
    /// Delimiter for NDJSON format (None for regular JSON)
    pub delimiter: Option<u8>,
    /// Schema URI to use ("AUTO" for auto-detection)
    pub schema_uri: Option<String>,
    /// Threshold above which non-fixed keys are treated as a map
    pub map_threshold: usize,
    /// Maximum number of required keys a Map can have. If None, no gating based on required keys.
    /// If Some(n), objects with more than n required keys will be forced to Record type.
    pub map_max_required_keys: Option<usize>,
    /// Enable unification of compatible but non-homogeneous record schemas into maps
    pub unify_maps: bool,
    /// Force override of field treatment, e.g. {"labels": "map"}
    pub force_field_types: HashMap<String, String>,
    /// Whether to promote scalar values to wrapped objects when they collide with record values
    /// during unification. If `true`, scalars are promoted under a synthetic property name derived from
    /// the parent field and the scalar type (e.g. "foo__string"). If `false`, don't unify on conflicts.
    pub wrap_scalars: bool,
    /// Wrap the inferred top-level schema under a single required field with this name.
    /// Example: wrap_root = Some("labels") turns `{...}` into
    /// `{"type":"object","properties":{"labels":{...}},"required":["labels"]}`.
    pub wrap_root: Option<String>,
    /// Whether to output Avro schema rather than regular JSON Schema.
    #[cfg(feature = "avro")]
    pub avro: bool,
    /// Enable debug output. When `true`, prints detailed information about schema inference
    /// processes including field unification, map detection, and scalar wrapping decisions.
    pub debug: bool,
    /// Controls the verbosity level of debug output
    pub verbosity: DebugVerbosity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DebugVerbosity {
    /// Show important unification decisions and failures  
    Normal,
    /// Show all debug information including field introductions
    Verbose,
}

impl Default for DebugVerbosity {
    fn default() -> Self {
        DebugVerbosity::Normal
    }
}

impl SchemaInferenceConfig {
    pub fn debug(&self, args: std::fmt::Arguments) {
        if self.debug {
            eprintln!("{}", args);
        }
    }

    pub fn debug_verbose(&self, args: std::fmt::Arguments) {
        if self.debug && matches!(self.verbosity, DebugVerbosity::Verbose) {
            eprintln!("{}", args);
        }
    }
}

impl Default for SchemaInferenceConfig {
    fn default() -> Self {
        Self {
            ignore_outer_array: true,
            delimiter: None,
            schema_uri: Some("AUTO".to_string()),
            map_threshold: 20,
            map_max_required_keys: None,
            unify_maps: false,
            force_field_types: std::collections::HashMap::new(),
            wrap_scalars: true,
            wrap_root: None,
            #[cfg(feature = "avro")]
            avro: false,
            debug: false,
            verbosity: DebugVerbosity::default(),
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($cfg:expr, $($arg:tt)*) => {
        $cfg.debug(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_verbose {
    ($cfg:expr, $($arg:tt)*) => {
        $cfg.debug_verbose(format_args!($($arg)*))
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInferenceResult {
    pub schema: Value,
    pub processed_count: usize,
}

#[cfg(feature = "avro")]
impl SchemaInferenceResult {
    pub fn to_avro_schema(
        &self,
        namespace: &str,
        utility_namespace: Option<&str>,
        base_uri: Option<&str>,
        split_top_level: bool,
    ) -> Value {
        avrotize::converter::jsons_to_avro(
            &self.schema,
            namespace,
            utility_namespace.unwrap_or(""),
            base_uri.unwrap_or("genson-core"),
            split_top_level,
        )
    }
}
