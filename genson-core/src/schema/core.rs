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
    /// Force parent objects containing these fields to remain as records, preventing map inference.
    /// e.g. {"mainsnak": "record"} prevents any object containing a "mainsnak" field from being
    /// converted to a map, ensuring homogeneity across array items.
    pub force_parent_field_types: HashMap<String, String>,
    /// Set of field names that should always be promoted to wrapped scalars,
    /// even when they appear as simple scalars (not in type unions). This ensures
    /// schema stability for fields known to have heterogeneous types across schematised files.
    pub force_scalar_promotion: std::collections::HashSet<String>,
    /// Whether to promote scalar values to wrapped objects when they collide with record values
    /// during unification, or with scalars of another kind (an integer and a string, not an
    /// integer and a number). If `true`, scalars are promoted under a synthetic property name
    /// derived from the parent field and the scalar type (e.g. "foo__string"), so every value
    /// keeps its type. If `false`, don't unify on conflicts.
    pub wrap_scalars: bool,
    /// Whether the rows' strings are to be coerced to the numbers or booleans beside them
    /// when normalised (`NormaliseConfig::coerce_string`): a string then does not clash with
    /// them, and is not given a field of its own (`wrap_scalars`). A configuration
    /// serialised before this field was added reads it as `false`.
    #[serde(default)]
    pub coerce_strings: bool,
    /// Wrap the inferred top-level schema under a single required field with this name.
    /// Example: wrap_root = Some("labels") turns `{...}` into
    /// `{"type":"object","properties":{"labels":{...}},"required":["labels"]}`.
    pub wrap_root: Option<String>,
    /// Prevent the document root from becoming a map type, even if it meets map inference criteria
    pub no_root_map: bool,
    /// Maximum number of schema builders to create in parallel at once
    /// Lower values reduce peak memory usage during schema inference
    /// None: process all strings at once
    pub max_builders: Option<usize>,
    /// Whether to output Avro schema rather than regular JSON Schema.
    #[cfg(feature = "avro")]
    pub avro: bool,
    /// Enable debug output. When `true`, prints detailed information about schema inference
    /// processes including field unification, map detection, and scalar wrapping decisions.
    pub debug: bool,
    /// Enable profiling output. When `true`, prints detailed information about timing.
    pub profile: bool,
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
    pub(crate) fn profile(&self, args: std::fmt::Arguments) {
        if self.profile {
            let message = format!("{}", args);
            anstream::eprintln!("{}", message);
        }
    }

    pub(crate) fn profile_verbose(&self, args: std::fmt::Arguments) {
        if self.profile && matches!(self.verbosity, DebugVerbosity::Verbose) {
            let message = format!("{}", args);
            anstream::eprintln!("{}", message);
        }
    }

    pub(crate) fn debug(&self, args: std::fmt::Arguments) {
        if self.debug {
            let message = format!("{}", args);
            anstream::eprintln!("{}", self.maybe_truncate(message));
        }
    }

    pub(crate) fn debug_verbose(&self, args: std::fmt::Arguments) {
        if self.debug_verbose_enabled() {
            let message = format!("{}", args);
            anstream::eprintln!("{}", self.maybe_truncate(message));
        }
    }

    /// Whether `debug_verbose` prints.
    pub(crate) fn debug_verbose_enabled(&self) -> bool {
        self.debug && matches!(self.verbosity, DebugVerbosity::Verbose)
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
            force_parent_field_types: std::collections::HashMap::new(),
            force_scalar_promotion: std::collections::HashSet::new(),
            wrap_scalars: true,
            coerce_strings: false,
            wrap_root: None,
            no_root_map: true,
            max_builders: None,
            #[cfg(feature = "avro")]
            avro: false,
            debug: false,
            profile: false,
            verbosity: DebugVerbosity::default(),
        }
    }
}

#[macro_export]
macro_rules! profile {
    ($cfg:expr, $($arg:tt)*) => {
        $cfg.profile(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! profile_verbose {
    ($cfg:expr, $($arg:tt)*) => {
        $cfg.profile_verbose(format_args!($($arg)*))
    };
}

// `format_args!` evaluates its arguments where it is written, so the macros check that the
// message prints before building it: an argument such as `serde_json::to_string(schema)`
// would otherwise run on every call with debugging off.

#[macro_export]
macro_rules! debug {
    ($cfg:expr, $($arg:tt)*) => {{
        let cfg = &$cfg;
        if cfg.debug {
            cfg.debug(format_args!($($arg)*))
        }
    }};
}

#[macro_export]
macro_rules! debug_verbose {
    ($cfg:expr, $($arg:tt)*) => {{
        let cfg = &$cfg;
        if cfg.debug_verbose_enabled() {
            cfg.debug_verbose(format_args!($($arg)*))
        }
    }};
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
        let mut schema = self.schema.clone();
        type_empty_array_items_as_null(&mut schema);
        keep_empty_objects_as_records(&mut schema);
        avrotize::converter::jsons_to_avro(
            &schema,
            namespace,
            utility_namespace.unwrap_or(""),
            base_uri.unwrap_or("genson-core"),
            split_top_level,
        )
    }
}

/// Give untyped array items (`"items": {}`, from arrays that were empty in every row)
/// the type `null`. Left untyped, avrotize expands them to a generic union whose first
/// non-null branch is `boolean`, so the column became `List(Boolean)` and could not be
/// concatenated with a batch where the array has values.
///
/// An untyped array in an `anyOf` with scalars instead takes their types: normalisation
/// wraps such a scalar into the array (its first branch), so typed `null` the array held
/// values it could not decode (`List(Null)` holding `["x"]`).
#[cfg(feature = "avro")]
fn type_empty_array_items_as_null(schema: &mut Value) {
    match schema {
        Value::Object(obj) => {
            if let Some(Value::Array(branches)) = obj.get_mut("anyOf") {
                type_empty_array_items_by_scalars(branches);
            }
            if is_untyped_array(obj) {
                obj.insert("items".to_string(), serde_json::json!({"type": "null"}));
            }
            obj.values_mut().for_each(type_empty_array_items_as_null);
        }
        Value::Array(arr) => arr.iter_mut().for_each(type_empty_array_items_as_null),
        _ => {}
    }
}

/// Give objects that were empty in every row (`{"type": "object"}`, alone or in a type
/// list, with no properties of any kind) an explicit `"properties": {}`. avrotize (from
/// 0.2.0) reads an object without `properties`, `patternProperties` or
/// `additionalProperties` as holding any properties: a map of its generic union. With
/// its properties given as none, the object stays a record with no fields
/// (`Struct({})`), as inferred, since no row had a key there.
#[cfg(feature = "avro")]
fn keep_empty_objects_as_records(schema: &mut Value) {
    match schema {
        Value::Object(obj) => {
            if is_empty_object(obj) {
                obj.insert("properties".to_string(), serde_json::json!({}));
            }
            obj.values_mut().for_each(keep_empty_objects_as_records);
        }
        Value::Array(arr) => arr.iter_mut().for_each(keep_empty_objects_as_records),
        _ => {}
    }
}

#[cfg(feature = "avro")]
fn is_empty_object(obj: &serde_json::Map<String, Value>) -> bool {
    let object = match obj.get("type") {
        Some(Value::String(t)) => t == "object",
        Some(Value::Array(types)) => types.iter().any(|t| t == "object"),
        _ => false,
    };
    object
        && !["properties", "patternProperties", "additionalProperties"]
            .iter()
            .any(|k| obj.contains_key(*k))
}

#[cfg(feature = "avro")]
fn is_untyped_array(obj: &serde_json::Map<String, Value>) -> bool {
    obj.get("type").and_then(|t| t.as_str()) == Some("array")
        && obj
            .get("items")
            .is_some_and(|i| i.as_object().is_some_and(|o| o.is_empty()))
}

/// The scalar types of an `anyOf`'s other branches (`null` aside) as the items of its
/// untyped arrays.
#[cfg(feature = "avro")]
fn type_empty_array_items_by_scalars(branches: &mut [Value]) {
    let mut scalars: Vec<Value> = Vec::new();
    for branch in branches.iter() {
        let types = match branch.get("type") {
            Some(Value::String(t)) => vec![t.as_str()],
            Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
            _ => continue,
        };
        // Only a branch that is scalars alone: one with any other keyword is not wrapped
        if branch.as_object().is_some_and(|o| o.len() != 1) {
            continue;
        }
        for t in types {
            let scalar = matches!(t, "string" | "integer" | "number" | "boolean");
            if scalar && !scalars.iter().any(|s| s == t) {
                scalars.push(Value::String(t.to_string()));
            }
        }
    }
    let items = match scalars.len() {
        0 => return,
        1 => serde_json::json!({"type": scalars[0]}),
        _ => serde_json::json!({"type": scalars}),
    };
    for branch in branches.iter_mut() {
        if let Some(obj) = branch.as_object_mut() {
            if is_untyped_array(obj) {
                obj.insert("items".to_string(), items.clone());
            }
        }
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
