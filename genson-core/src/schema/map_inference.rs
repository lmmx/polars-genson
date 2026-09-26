// genson-core/src/schema/map_inference.rs
use crate::schema::core::{make_promoted_scalar_key, SchemaInferenceConfig};
use crate::{debug, profile_verbose};
use rayon::prelude::*;
use serde_json::Value;
mod unification;
use super::current_time_hms;
use unification::*;

const PARALLEL_PROP_THRESHOLD: usize = 3;

/// Process properties in parallel when beneficial
fn process_properties_parallel<F>(
    props_obj: &mut serde_json::Map<String, Value>,
    config: &SchemaInferenceConfig,
    processor: F,
) where
    F: Fn(&str, &mut Value) + Send + Sync,
{
    if props_obj.len() >= PARALLEL_PROP_THRESHOLD {
        profile_verbose!(
            config,
            "Parallelising: {} properties ({})",
            props_obj.len(),
            current_time_hms()
        );
        // Extract entries, process in parallel, reconstruct map
        let entries: Vec<_> = std::mem::take(props_obj).into_iter().collect();
        let processed: Vec<(String, Value)> = entries
            .into_par_iter()
            .map(|(k, mut v)| {
                processor(&k, &mut v);
                (k, v)
            })
            .collect();
        *props_obj = processed.into_iter().collect();
    } else {
        // Sequential for small property sets
        for (k, v) in props_obj.iter_mut() {
            processor(k, v);
        }
    }
}

/// The non-null schema of a nullable schema (`{"type": ["null", T]}` or the legacy
/// `["null", {...}]`), without copying it: a schema plus the `type` that replaces its own.
struct NonNullView<'a> {
    schema: &'a Value,
    type_override: Option<&'a Value>,
}

fn non_null_view(schema: &Value) -> NonNullView<'_> {
    let null = Value::String("null".into());
    if let Some(Value::Array(type_arr)) = schema.get("type") {
        if type_arr.len() == 2 && type_arr.contains(&null) {
            return NonNullView {
                schema,
                type_override: type_arr.iter().find(|t| **t != null),
            };
        }
    }
    if let Value::Array(arr) = schema {
        if arr.len() == 2 && arr.contains(&null) {
            if let Some(inner) = arr.iter().find(|v| **v != null) {
                return NonNullView {
                    schema: inner,
                    type_override: None,
                };
            }
        }
    }
    NonNullView {
        schema,
        type_override: None,
    }
}

impl NonNullView<'_> {
    fn field<'b>(&'b self, key: &str, value: &'b Value) -> &'b Value {
        match self.type_override {
            Some(t) if key == "type" => t,
            _ => value,
        }
    }

    /// Equality of the two non-null schemas.
    fn eq(&self, other: &NonNullView) -> bool {
        if self.type_override.is_none() && other.type_override.is_none() {
            return self.schema == other.schema;
        }
        let (Value::Object(a), Value::Object(b)) = (self.schema, other.schema) else {
            return false;
        };
        a.len() == b.len()
            && a.iter().all(|(k, v)| {
                b.get(k)
                    .is_some_and(|w| self.field(k, v) == other.field(k, w))
            })
    }

    fn to_value(&self) -> Value {
        let mut schema = self.schema.clone();
        if let (Some(t), Some(obj)) = (self.type_override, schema.as_object_mut()) {
            obj.insert("type".to_string(), t.clone());
        }
        schema
    }
}

fn contains_anyof(value: &Value) -> bool {
    match value {
        Value::Object(obj) => {
            if obj.contains_key("anyOf") {
                return true;
            }
            obj.values().any(contains_anyof)
        }
        Value::Array(arr) => arr.iter().any(contains_anyof),
        _ => false,
    }
}

/// Value schema for an object forced to be a map: the values' common schema when they
/// share one (unified if they differ, unless a key is in `no_unify`), otherwise string,
/// which holds every value as text so none is lost.
pub(crate) fn forced_map_value_schema(
    obj: &serde_json::Map<String, Value>,
    field_name: Option<&str>,
    config: &SchemaInferenceConfig,
) -> Value {
    let string = || serde_json::json!({ "type": "string" });
    if let Some(values) = obj.get("additionalProperties").filter(|v| v.is_object()) {
        return values.clone();
    }
    let Some(props) = obj
        .get("properties")
        .and_then(|p| p.as_object())
        .filter(|p| !p.is_empty())
    else {
        return string();
    };
    let first = non_null_view(props.values().next().unwrap());
    if props.values().all(|v| non_null_view(v).eq(&first)) {
        return first.to_value();
    }
    if props.keys().any(|k| config.no_unify.contains(k.as_str())) {
        return string();
    }
    let schemas: Vec<&Value> = props.values().collect();
    check_unifiable_schemas(&schemas, field_name.unwrap_or(""), config).unwrap_or_else(string)
}

/// Process anyOf unions in a schema recursively
fn process_anyof_unions(
    schema: &mut Value,
    field_name: &str,
    config: &SchemaInferenceConfig,
) -> bool {
    let mut made_changes = false;

    match schema {
        Value::Object(obj) => {
            // Handle direct anyOf at this level
            // Scalar/object unions are promoted whenever wrap_scalars is set (checked in
            // unify_anyof_schemas), independent of unify_maps
            if let Some(Value::Array(any_of_schemas)) = obj.get("anyOf") {
                let any_of_refs: Vec<&Value> = any_of_schemas.iter().collect();
                if let Some(unified) = unify_anyof_schemas(&any_of_refs, field_name, config) {
                    debug!(config, "Successfully unified anyOf schemas");
                    *schema = unified;
                    made_changes = true;
                    // Recursively process the newly unified schema
                    if process_anyof_unions(schema, field_name, config) {
                        made_changes = true;
                    }
                    return made_changes;
                }
            }

            // Recursively process all nested values - pass field names for known properties
            if let Some(props) = obj.get_mut("properties") {
                if let Some(props_obj) = props.as_object_mut() {
                    for (k, v) in props_obj {
                        if process_anyof_unions(v, k, config) {
                            made_changes = true;
                        }
                    }
                }
            } else {
                // For other nested values, use the current field name
                for v in obj.values_mut() {
                    if process_anyof_unions(v, field_name, config) {
                        made_changes = true;
                    }
                }
            }
        }
        Value::Array(arr) => {
            for v in arr {
                if process_anyof_unions(v, field_name, config) {
                    made_changes = true;
                }
            }
        }
        _ => {}
    }

    made_changes
}

/// Check if an object schema contains any fields specified in force_parent_field_types.
/// Returns Some(forced_type) if a match is found, None otherwise.
fn check_force_parent_field_types<'a>(
    obj: &serde_json::Map<String, Value>,
    config: &'a SchemaInferenceConfig,
) -> Option<&'a str> {
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        for (prop_key, forced_parent_type) in &config.force_parent_field_types {
            if props.contains_key(prop_key) {
                return Some(forced_parent_type.as_str());
            }
        }
    }
    None
}

/// Post-process an inferred JSON Schema to rewrite certain object shapes as maps.
///
/// This mutates the schema in place, applying user overrides and heuristics.
///
/// # Rules
/// - If the current field name matches a `force_field_types` override, that wins
///   (`"map"` rewrites to `additionalProperties` with the values' common schema,
///   `"record"` leaves as-is).
/// - Otherwise, applies map inference heuristics based on:
///   - Total key cardinality (`map_threshold`)
///   - Required key cardinality (`map_max_required_keys`)
///   - Value homogeneity (all values must be homogeneous) OR
///   - Value unifiability (compatible record schemas when `unify_maps` enabled)
/// - Recurses into nested objects/arrays, carrying field names down so overrides apply.
pub(crate) fn rewrite_objects(
    schema: &mut Value,
    field_name: Option<&str>,
    config: &SchemaInferenceConfig,
    is_root: bool,
) {
    if config.debug {
        debug!(
            config,
            "rewrite_objects(field_name={:?}, schema={})",
            field_name,
            serde_json::to_string(schema).unwrap_or_default()
        );
    }
    // GUARD: Skip map conversion if this field was force-promoted to a scalar wrapper
    // BUT: Allow explicit force_field_types to override
    if let Some(name) = field_name {
        if config.force_scalar_promotion.contains(name)
            && !config.force_field_types.contains_key(name)
        {
            debug!(
                config,
                "Skipping map conversion for force-promoted field '{}'", name
            );
            // Check if this is a scalar type that needs promotion
            if let Some(type_val) = schema.get("type") {
                if let Some(type_str) = type_val.as_str() {
                    if matches!(type_str, "string" | "integer" | "number" | "boolean") {
                        debug!(
                            config,
                            "Force promoting scalar field '{}' of type '{}'", name, type_str
                        );

                        let wrapped_key = make_promoted_scalar_key(name, type_str);
                        let scalar_schema = schema.clone();

                        *schema = serde_json::json!({
                            "type": "object",
                            "properties": {
                                wrapped_key: scalar_schema
                            }
                        });
                    }
                } else if let Some(type_arr) = type_val.as_array() {
                    if type_arr.len() == 2 && type_arr.contains(&Value::String("null".into())) {
                        if let Some(inner_type) = type_arr
                            .iter()
                            .find(|t| *t != &Value::String("null".into()))
                            .and_then(|t| t.as_str())
                        {
                            if matches!(inner_type, "string" | "integer" | "number" | "boolean") {
                                debug!(
                                    config,
                                    "Force promoting nullable scalar field '{}' of type '{}'",
                                    name,
                                    inner_type
                                );

                                let wrapped_key = make_promoted_scalar_key(name, inner_type);
                                let scalar_schema = schema.clone();

                                *schema = serde_json::json!({
                                    "type": "object",
                                    "properties": {
                                        wrapped_key: scalar_schema
                                    }
                                });
                                return;
                            }
                        }
                    }
                } else if let Some(type_arr) = type_val.as_array() {
                    if type_arr.len() == 2 && type_arr.contains(&Value::String("null".into())) {
                        if let Some(inner_type) = type_arr
                            .iter()
                            .find(|t| *t != &Value::String("null".into()))
                            .and_then(|t| t.as_str())
                        {
                            if matches!(inner_type, "string" | "integer" | "number" | "boolean") {
                                debug!(
                                    config,
                                    "Force promoting nullable scalar field '{}' of type '{}'",
                                    name,
                                    inner_type
                                );

                                let wrapped_key = make_promoted_scalar_key(name, inner_type);
                                let scalar_schema = schema.clone();

                                *schema = serde_json::json!({
                                    "type": "object",
                                    "properties": {
                                        wrapped_key: scalar_schema
                                    }
                                });
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Value::Object(obj) = schema {
        // --- Forced overrides by field name ---
        if let Some(name) = field_name {
            if let Some(forced) = config.force_field_types.get(name) {
                if config.debug {
                    debug!(config, "Hit force field: {}={}", name, forced);
                }
                match forced.as_str() {
                    "map" => {
                        let mut values = forced_map_value_schema(obj, field_name, config);
                        // As for inferred maps, the value schema's own objects get rewritten
                        rewrite_objects(&mut values, None, config, false);
                        obj.shift_remove("properties");
                        obj.shift_remove("required");
                        obj.insert("additionalProperties".to_string(), values);
                        return; // no need to apply heuristics
                    }
                    "record" => {
                        if let Some(props) =
                            obj.get_mut("properties").and_then(|p| p.as_object_mut())
                        {
                            process_properties_parallel(props, config, |k, v| {
                                if config.debug {
                                    debug!(config, "Force field induced recursion: {}", k);
                                }
                                rewrite_objects(v, Some(k), config, false);
                            });
                        }
                        if let Some(items) = obj.get_mut("items") {
                            debug!(config, "Force field induced recursion: items");
                            rewrite_objects(items, None, config, false);
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }

        // --- Handle anyOf unions ---
        // Scalar/object unions are promoted whenever wrap_scalars is set (checked in
        // unify_anyof_schemas), independent of unify_maps
        if let Some(Value::Array(any_of_schemas)) = obj.get("anyOf") {
            if config.debug {
                debug!(
                    config,
                    "Found anyOf union with {} schemas, attempting unification",
                    any_of_schemas.len()
                );
            }
            let any_of_refs: Vec<&Value> = any_of_schemas.iter().collect();
            if let Some(unified) =
                unify_anyof_schemas(&any_of_refs, field_name.unwrap_or(""), config)
            {
                debug!(config, "Successfully unified anyOf schemas");
                // Replace the entire schema with the unified result
                *schema = unified;
                // Recurse into the unified schema to apply further processing
                rewrite_objects(schema, field_name, config, is_root);
                return;
            } else {
                debug!(config, "Failed to unify anyOf schemas, leaving as-is");
            }
            // If unification disabled or failed, still recurse into each anyOf branch
            if let Some(any_of_array) = obj.get_mut("anyOf").and_then(|a| a.as_array_mut()) {
                if any_of_array.len() >= 3 {
                    any_of_array.par_iter_mut().for_each(|any_of_schema| {
                        rewrite_objects(any_of_schema, field_name, config, false);
                    });
                } else {
                    for any_of_schema in any_of_array {
                        rewrite_objects(any_of_schema, field_name, config, false);
                    }
                }
            }
        }

        // --- Heuristic rewrite ---
        if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
            // GUARD: Check if this object contains force_parent_field_types keys
            if let Some(forced_parent_type) = check_force_parent_field_types(obj, config) {
                if config.debug {
                    debug!(
                        config,
                        "Object at field {:?} contains a force_parent_field, forcing parent type to '{}'",
                        field_name.unwrap_or("root"),
                        forced_parent_type
                    );
                }

                if forced_parent_type == "record" {
                    // Skip map conversion, but still recurse into properties
                    if let Some(props_mut) =
                        obj.get_mut("properties").and_then(|p| p.as_object_mut())
                    {
                        process_properties_parallel(props_mut, config, |k, v| {
                            if config.debug {
                                debug!(config, "Force parent field induced recursion: {}", k);
                            }
                            rewrite_objects(v, Some(k), config, false);
                        });
                    }
                    if let Some(items) = obj.get_mut("items") {
                        debug!(config, "Force parent field induced recursion: items");
                        rewrite_objects(items, None, config, false);
                    }
                    return;
                }
                // If "map" is forced, continue with normal map conversion logic below
            }

            // GUARD: Skip map conversion if this field was force-promoted to a scalar wrapper
            if let Some(name) = field_name {
                if config.force_scalar_promotion.contains(name) {
                    debug!(
                        config,
                        "Skipping map conversion for force-promoted field '{}'", name
                    );
                    // Still need to recurse into properties
                    if let Some(props_mut) =
                        obj.get_mut("properties").and_then(|p| p.as_object_mut())
                    {
                        process_properties_parallel(props_mut, config, |k, v| {
                            rewrite_objects(v, Some(k), config, false);
                        });
                    }
                    return;
                }
            }
            // GUARD: Skip re-processing of already converted map schemas
            if obj.get("additionalProperties").is_some() {
                if props.is_empty() {
                    if config.debug {
                        debug!(
                            config,
                            "Skipping re-processing of already converted map schema at field {:?}",
                            field_name.unwrap_or("root")
                        );
                    }
                    // Just recurse into the additionalProperties value and return
                    if let Some(additional_props) = obj.get_mut("additionalProperties") {
                        if config.debug {
                            debug!(
                                config,
                                "Rewriting already converted map schema at field {:?}",
                                field_name.unwrap_or("root")
                            );
                        }
                        // Hmm: shouldn't this be `field_name` not None?
                        rewrite_objects(additional_props, None, config, false);
                    }
                    return;
                } else {
                    // This shouldn't happen - schema shouldn't have both props + additionalProperties
                    if config.debug {
                        debug!(
                            config,
                            "Warning: schema has both properties and additionalProperties at field {:?}",
                            field_name.unwrap_or("root")
                        );
                    }
                }
            }
            let key_count = props.len(); // |UK| - total keys observed
            let above_threshold = key_count >= config.map_threshold;

            // Copy out child schema shapes
            let child_schemas: Vec<&Value> = props.values().collect();
            if config.profile && child_schemas.len() > 50 {
                anstream::eprintln!("Collected {} schemas", child_schemas.len());
            }

            // Detect map-of-records only if:
            // - all children are identical
            // - and that child is itself an object with "properties" (i.e. a proper record)
            if above_threshold && !(is_root && config.no_root_map) {
                if let Some(first) = child_schemas.first() {
                    if first.get("type") == Some(&Value::String("object".into()))
                        && first.get("properties").is_some()
                        && child_schemas.len() > 1
                    {
                        let all_same = child_schemas.par_iter().all(|other| other == first);
                        if all_same {
                            let mut first_clone = (*first).clone();
                            obj.shift_remove("properties");
                            obj.shift_remove("required");
                            // As for the map branch below: the record's own fields get rewritten
                            rewrite_objects(&mut first_clone, None, config, false);
                            obj.insert("additionalProperties".to_string(), first_clone);
                            return;
                        }
                    }
                }
            }

            // Calculate required key count |RK|
            let required_key_count = obj
                .get("required")
                .and_then(|r| r.as_array())
                .map(|r| r.len())
                .unwrap_or(0);

            // Check for unifiable schemas
            let mut unified_schema: Option<Value> = None;
            if let Some(first_schema) = props.values().next() {
                // Normalise all schemas for comparison
                // Borrowed, since only the first is ever copied (when they all match)
                let normalised_schemas: Vec<NonNullView> =
                    props.values().map(non_null_view).collect();
                let first_view = non_null_view(first_schema);

                // Debug output to diagnose the issue
                if config.debug {
                    debug!(
                        config,
                        "Checking homogeneity for field {:?} with {} schemas",
                        field_name.unwrap_or("root"),
                        normalised_schemas.len()
                    );

                    let mut unique_schemas = std::collections::BTreeSet::new();
                    for view in &normalised_schemas {
                        let schema = view.to_value();
                        unique_schemas.insert(serde_json::to_string(&schema).unwrap_or_default());
                    }

                    if unique_schemas.len() <= 3 {
                        // Only show details for small sets
                        debug!(
                            config,
                            "Unique normalised schemas ({} total):",
                            unique_schemas.len()
                        );
                        for (i, schema_str) in unique_schemas.iter().enumerate() {
                            if schema_str.len() > 300 {
                                // Parse back to Value and pretty-print
                                if let Ok(schema_value) = serde_json::from_str::<Value>(schema_str)
                                {
                                    let pretty_schema = serde_json::to_string_pretty(&schema_value)
                                        .unwrap_or_default();
                                    let lines: Vec<&str> = pretty_schema.lines().collect();
                                    if lines.len() > 12 {
                                        let first_6 = lines[..6].join("\n");
                                        let last_6 = lines[lines.len() - 6..].join("\n");
                                        debug!(
                                            config,
                                            "  Schema {}:\n{}\n...({} lines omitted)...\n{}",
                                            i,
                                            first_6,
                                            lines.len() - 12,
                                            last_6
                                        );
                                    } else {
                                        debug!(config, "  Schema {}:\n{}", i, pretty_schema);
                                    }
                                } else {
                                    debug!(config, "  Schema {}: [invalid JSON]", i);
                                }
                            } else {
                                debug!(config, "  Schema {}: {}", i, schema_str);
                            }
                        }
                    } else {
                        debug!(
                            config,
                            "Found {} unique normalised schemas (too many to display)",
                            unique_schemas.len()
                        );
                    }
                }

                let homog_start = std::time::Instant::now();
                if normalised_schemas
                    .par_iter()
                    .all(|view| view.eq(&first_view))
                {
                    // All schemas are homogeneous after normalisation
                    debug!(config, "Schemas are homogeneous after normalisation");
                    unified_schema = Some(first_view.to_value());
                } else if config.unify_maps {
                    debug!(config, "Schemas not homogeneous, attempting unification");
                    if config.profile && normalised_schemas.len() > 50 {
                        anstream::eprintln!(
                            "Unification of {} heterogeneous schemas beginning...",
                            normalised_schemas.len()
                        );
                    }

                    // Check if any of the property keys are in no_unify
                    let has_excluded_field =
                        props.keys().any(|k| config.no_unify.contains(k.as_str()));
                    if has_excluded_field {
                        if config.debug {
                            debug!(
                                config,
                                "Not unifying: one or more fields in no_unify: {:?}",
                                props
                                    .keys()
                                    .filter(|k| config.no_unify.contains(k.as_str()))
                                    .collect::<Vec<_>>()
                            );
                        }
                    } else {
                        // Detect if these are all arrays of records
                        if child_schemas
                            .par_iter()
                            .all(|s| s.get("type") == Some(&Value::String("array".into())))
                        {
                            // Collect item schemas, short-circuit if any missing
                            let mut item_schemas: Vec<&Value> =
                                Vec::with_capacity(child_schemas.len());
                            let mut all_items_ok = true;
                            for s in &child_schemas {
                                if let Some(items) = s.get("items") {
                                    item_schemas.push(items);
                                } else {
                                    all_items_ok = false;
                                    break;
                                }
                            }
                            if all_items_ok {
                                let unify_start = std::time::Instant::now();
                                if let Some(unified_items) = check_unifiable_schemas(
                                    &item_schemas,
                                    field_name.unwrap_or(""),
                                    config,
                                ) {
                                    unified_schema = Some(serde_json::json!({
                                        "type": "array",
                                        "items": unified_items
                                    }));
                                }
                                if config.profile && child_schemas.len() > 50 {
                                    anstream::eprintln!(
                                        "Unification of {} item schemas took {:?}",
                                        child_schemas.len(),
                                        unify_start.elapsed()
                                    );
                                }
                            }
                        } else {
                            // Only try record unification if unify_maps is enabled and above threshold
                            // This ensures we only do expensive unification when it would result in map conversion
                            if above_threshold {
                                let unify_start = std::time::Instant::now();
                                unified_schema = check_unifiable_schemas(
                                    &child_schemas,
                                    field_name.unwrap_or(""),
                                    config,
                                );
                                if config.profile && child_schemas.len() > 50 {
                                    anstream::eprintln!(
                                        "Unification of {} child schemas took {:?}",
                                        child_schemas.len(),
                                        unify_start.elapsed()
                                    );
                                }
                            }
                        }
                    }
                }
                if config.profile && normalised_schemas.len() > 50 {
                    anstream::eprintln!(
                        "Homogeneity check on {} schemas took {:?}",
                        normalised_schemas.len(),
                        homog_start.elapsed()
                    );
                }
            }

            // Process anyOf unions in the unified schema before checking for map conversion
            if let Some(ref mut schema) = unified_schema {
                if contains_anyof(schema) {
                    debug!(
                        config,
                        "Unified schema contains anyOf, processing unions first"
                    );
                    process_anyof_unions(schema, field_name.unwrap_or(""), config);
                }
            }

            // Apply map inference logic
            let should_be_map = if above_threshold && unified_schema.is_some() {
                debug!(
                    config,
                    "Checking if should convert to map: above_threshold=true, unified_schema=Some",
                );

                // Skip map inference if this is the root and no_root_map is enabled
                if is_root && config.no_root_map {
                    debug!(
                        config,
                        "Skipping map conversion: is root and no_root_map=true"
                    );
                    false
                } else if let Some(max_required) = config.map_max_required_keys {
                    let result = required_key_count <= max_required;
                    if config.debug {
                        debug!(
                            config,
                            "Map conversion decision: required_keys={} <= max_required={} = {}",
                            required_key_count,
                            max_required,
                            result
                        );
                    }
                    result
                } else {
                    debug!(
                        config,
                        "Map conversion: no max_required_keys limit, converting to map"
                    );
                    true
                }
            } else {
                if !above_threshold {
                    if config.debug {
                        debug!(
                            config,
                            "Not converting to map: below threshold ({} < {})",
                            key_count,
                            config.map_threshold
                        );
                    }
                } else if unified_schema.is_none() {
                    debug!(config, "Not converting to map: no unified schema");
                }
                false
            };

            if should_be_map {
                if let Some(schema) = unified_schema {
                    if config.debug {
                        let pretty_schema =
                            serde_json::to_string_pretty(&schema).unwrap_or_default();
                        let lines: Vec<&str> = pretty_schema.lines().collect();
                        if lines.len() > 12 {
                            let first_6 = lines[..6].join("\n");
                            let last_6 = lines[lines.len() - 6..].join("\n");
                            debug!(config, "Converting field {:?} to map with schema:\n{}\n...({} lines omitted)...\n{}", 
                                   field_name.unwrap_or("root"), first_6, lines.len() - 12, last_6);
                        } else {
                            debug!(
                                config,
                                "Converting field {:?} to map with schema:\n{}",
                                field_name.unwrap_or("root"),
                                pretty_schema
                            );
                        }
                    }

                    obj.shift_remove("properties");
                    obj.shift_remove("required");
                    obj.insert("type".to_string(), Value::String("object".to_string()));

                    // Process the schema being moved to additionalProperties for nested anyOf
                    let mut processed_schema = schema.clone();
                    rewrite_objects(&mut processed_schema, None, config, false);
                    obj.insert("additionalProperties".to_string(), processed_schema);

                    return;
                }
            }
        }

        // Skip recursion if we have a field name that's in the force types map
        if !matches!(field_name, Some(name) if config.force_field_types.contains_key(name)) {
            // --- Recurse into nested values ---
            if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
                process_properties_parallel(props, config, |k, v| {
                    if config.debug {
                        debug!(config, "Nested value recursion: {}", k);
                    }
                    rewrite_objects(v, Some(k), config, false);
                });
            }
            if let Some(items) = obj.get_mut("items") {
                debug!(config, "Nested value recursion: items");
                rewrite_objects(items, None, config, false);
            }
            for (k, v) in obj.iter_mut() {
                if matches!(
                    k.as_str(),
                    "properties" | "items" | "type" | "required" | "$schema" | "namespace" | "name"
                ) {
                    continue;
                }
                if let Value::Object(_) = v {
                    if config.debug {
                        debug!(config, "Other value recursion: {}", k);
                    }
                    rewrite_objects(v, Some(k), config, false);
                }
            }
        }
    } else if let Value::Array(arr) = schema {
        for v in arr {
            debug!(config, "Array value recursion");
            rewrite_objects(v, None, config, false);
        }
    }
}
