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
    /// Fields whose keys should not be merged during record unification
    pub no_unify: std::collections::HashSet<String>,
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
    /// Prevent the document root from becoming a map type, even if it meets map inference criteria
    pub no_root_map: bool,
    /// Whether to output Avro schema rather than regular JSON Schema.
    #[cfg(feature = "avro")]
    pub avro: bool,
    /// Enable debug output. When `true`, prints detailed information about schema inference
    /// processes including field unification, map detection, and scalar wrapping decisions.
    pub debug: bool,
    /// Controls the verbosity level of debug output
    pub verbosity: DebugVerbosity,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DebugVerbosity {
    /// Show important unification decisions and failures  
    #[default]
    Normal,
    /// Show all debug information including field introductions
    Verbose,
}

impl SchemaInferenceConfig {
    pub(crate) fn debug(&self, args: std::fmt::Arguments) {
        if self.debug {
            let message = format!("{}", args);
            eprintln!("{}", self.maybe_truncate(message));
        }
    }

    pub(crate) fn debug_verbose(&self, args: std::fmt::Arguments) {
        if self.debug && matches!(self.verbosity, DebugVerbosity::Verbose) {
            let message = format!("{}", args);
            eprintln!("{}", self.maybe_truncate(message));
        }
    }

    fn maybe_truncate(&self, message: String) -> String {
        let lines: Vec<&str> = message.lines().collect();

        if lines.len() > 20 && self.verbosity == DebugVerbosity::Normal {
            let mut truncated = String::new();

            // First 10 lines
            for line in lines.iter().take(10) {
                truncated.push_str(line);
                truncated.push('\n');
            }

            truncated.push_str(&format!("... ({} lines truncated) ...\n", lines.len() - 15));

            // Last 5 lines
            for line in lines.iter().skip(lines.len() - 5) {
                truncated.push_str(line);
                truncated.push('\n');
            }

            truncated
        } else {
            message
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
            no_unify: std::collections::HashSet::new(),
            force_field_types: std::collections::HashMap::new(),
            wrap_scalars: true,
            wrap_root: None,
            no_root_map: true,
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

/// Generate a consistent key name for promoted scalar values.
///
/// Creates keys in the format `{field_prefix}__{scalar_type}` for scalar values
/// that are promoted to object fields during schema unification or normalisation.
pub fn make_promoted_scalar_key(field_prefix: &str, scalar_type: &str) -> String {
    // Could be parameterised by config in future to make configurable
    format!("{}__{}", field_prefix, scalar_type)
}
