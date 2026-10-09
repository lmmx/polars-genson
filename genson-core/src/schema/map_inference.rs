// genson-core/src/schema/map_inference.rs
use crate::schema::core::{make_promoted_scalar_key, SchemaInferenceConfig};
use crate::{debug, profile_verbose};
use rayon::prelude::*;
use serde_json::Value;
mod unification;
use super::current_time_hms;
use unification::*;

const PARALLEL_PROP_THRESHOLD: usize = 3;

/// Call `processor` on every property, on the rayon pool when there are enough of them.
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
        let entries: Vec<(&String, &mut Value)> = props_obj.iter_mut().collect();
        entries.into_par_iter().for_each(|(k, v)| processor(k, v));
    } else {
        for (k, v) in props_obj.iter_mut() {
            processor(k, v);
        }
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
/// which holds every value as text so none is lost. An object empty in every row has no
/// values to type: null, as for an always-empty array's items.
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
        return serde_json::json!({ "type": "null" });
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

/// Whether a schema's `type` is `"object"`, or `["null", "object"]`.
fn is_object_type(t: Option<&Value>) -> bool {
    match t {
        Some(Value::String(s)) => s == "object",
        Some(Value::Array(ts)) => {
            ts.len() == 2 && ts.iter().any(|t| t == "null") && ts.iter().any(|t| t == "object")
        }
        _ => false,
    }
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
/// - A field in `force_scalar_promotion` holding a scalar becomes a record of that scalar.
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
    if promote_forced_scalar(schema, field_name, config) {
        return;
    }
    let obj = match schema {
        Value::Object(obj) => obj,
        Value::Array(items) => {
            for item in items {
                debug!(config, "Array value recursion");
                rewrite_objects(item, None, config, false);
            }
            return;
        }
        _ => return,
    };

    // --- Forced overrides by field name ---
    let forced = field_name.and_then(|name| config.force_field_types.get(name));
    if let Some(forced) = forced {
        debug!(config, "Hit force field: {:?}={}", field_name, forced);
        match forced.as_str() {
            "map" => {
                let mut values = forced_map_value_schema(obj, field_name, config);
                // As for inferred maps, the value schema's own objects get rewritten
                rewrite_objects(&mut values, None, config, false);
                obj.shift_remove("properties");
                obj.shift_remove("required");
                obj.insert("additionalProperties".to_string(), values);
                return;
            }
            "record" => {
                rewrite_fields(obj, config);
                return;
            }
            _ => {}
        }
    }

    // --- anyOf unions: scalars among records are promoted (when `wrap_scalars` is set) ---
    if let Some(Value::Array(any_of)) = obj.get("anyOf") {
        debug!(
            config,
            "Found anyOf union with {} schemas, attempting unification",
            any_of.len()
        );
        let any_of: Vec<&Value> = any_of.iter().collect();
        if let Some(unified) = unify_anyof_schemas(&any_of, field_name.unwrap_or(""), config) {
            debug!(config, "Successfully unified anyOf schemas");
            *schema = unified;
            rewrite_objects(schema, field_name, config, is_root);
            return;
        }
        debug!(config, "Failed to unify anyOf schemas, leaving as-is");
        if let Some(Value::Array(any_of)) = obj.get_mut("anyOf") {
            let rewrite = |branch: &mut Value| rewrite_objects(branch, field_name, config, false);
            if any_of.len() >= 3 {
                any_of.par_iter_mut().for_each(rewrite);
            } else {
                any_of.iter_mut().for_each(rewrite);
            }
        }
    }

    // --- An object empty in every row, where every object is a map candidate ---
    // A record with no fields cannot be written (Parquet has no empty struct); as a
    // map it has no values to type, so null, as for an always-empty array's items
    if config.map_threshold == 0
        && !(is_root && config.no_root_map)
        && is_object_type(obj.get("type"))
        && obj.get("additionalProperties").is_none()
        && obj
            .get("properties")
            .and_then(|p| p.as_object())
            .is_none_or(|p| p.is_empty())
    {
        debug!(
            config,
            "Always-empty object at field {:?} becomes a map of null",
            field_name.unwrap_or("root")
        );
        obj.shift_remove("properties");
        obj.shift_remove("required");
        obj.insert(
            "additionalProperties".to_string(),
            serde_json::json!({ "type": "null" }),
        );
        return;
    }

    // --- Heuristic rewrite ---
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        if check_force_parent_field_types(obj, config) == Some("record") {
            debug!(
                config,
                "Object at field {:?} contains a force_parent_field, keeping it a record",
                field_name.unwrap_or("root")
            );
            rewrite_fields(obj, config);
            return;
        }
        // An object holding a force-promoted field is a record: the field is promoted to
        // an object within it, so its scalar schema here must not make the fields look
        // homogeneous (all strings) and the object a map. Nor does a force-promoted field
        // become a map itself.
        let promoted = |name: &str| config.force_scalar_promotion.contains(name);
        let holds_promoted = props.keys().any(|k| promoted(k));
        if field_name.is_some_and(promoted) || holds_promoted {
            debug!(
                config,
                "Object at field {:?} is or holds a force-promoted field, keeping it a record",
                field_name.unwrap_or("root")
            );
            rewrite_fields(obj, config);
            return;
        }
        if obj.get("additionalProperties").is_some() {
            if props.is_empty() {
                // Already a map: only its value schema is left to rewrite
                if let Some(values) = obj.get_mut("additionalProperties") {
                    rewrite_objects(values, None, config, false);
                }
                return;
            }
            debug!(
                config,
                "Warning: schema has both properties and additionalProperties at field {:?}",
                field_name.unwrap_or("root")
            );
        }

        let may_be_map = props.len() >= config.map_threshold && !(is_root && config.no_root_map);
        if let Some(mut values) = may_be_map.then(|| identical_record_values(props)).flatten() {
            obj.shift_remove("properties");
            obj.shift_remove("required");
            // As for the map branch below: the record's own fields get rewritten
            rewrite_objects(&mut values, None, config, false);
            obj.insert("additionalProperties".to_string(), values);
            return;
        }
        let required_key_count = obj
            .get("required")
            .and_then(|r| r.as_array())
            .map_or(0, |r| r.len());
        let few_required = config
            .map_max_required_keys
            .is_none_or(|max| required_key_count <= max);
        if !(may_be_map && few_required) {
            debug!(
                config,
                "Not converting field {:?} to map: {} keys (threshold {}), {} required (max {:?})",
                field_name.unwrap_or("root"),
                props.len(),
                config.map_threshold,
                required_key_count,
                config.map_max_required_keys
            );
        } else if let Some(mut values) = unified_values(props, field_name, config) {
            debug!(
                config,
                "Converting field {:?} to map with schema:\n{}",
                field_name.unwrap_or("root"),
                serde_json::to_string_pretty(&values).unwrap_or_default()
            );
            obj.shift_remove("properties");
            obj.shift_remove("required");
            obj.insert("type".to_string(), Value::String("object".to_string()));
            // The value schema's own objects get rewritten too
            rewrite_objects(&mut values, None, config, false);
            obj.insert("additionalProperties".to_string(), values);
            return;
        }
    }

    // A field forced to some other type is left as it is
    if forced.is_some() {
        return;
    }
    // --- Recurse into nested values ---
    rewrite_fields(obj, config);
    for (k, v) in obj.iter_mut() {
        if matches!(
            k.as_str(),
            "properties" | "items" | "type" | "required" | "$schema" | "namespace" | "name"
        ) {
            continue;
        }
        if let Value::Object(_) = v {
            debug!(config, "Other value recursion: {}", k);
            rewrite_objects(v, Some(k), config, false);
        }
    }
}

/// Rewrite the objects within a record's fields (and within an array's items).
fn rewrite_fields(obj: &mut serde_json::Map<String, Value>, config: &SchemaInferenceConfig) {
    if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
        process_properties_parallel(props, config, |k, v| {
            debug!(config, "Nested value recursion: {}", k);
            rewrite_objects(v, Some(k), config, false);
        });
    }
    if let Some(items) = obj.get_mut("items") {
        debug!(config, "Nested value recursion: items");
        rewrite_objects(items, None, config, false);
    }
}

/// A field in `force_scalar_promotion` (and not in `force_field_types`) holding a scalar,
/// nullable or not, becomes a record of that scalar under its promoted key. Returns whether
/// it did.
fn promote_forced_scalar(
    schema: &mut Value,
    field_name: Option<&str>,
    config: &SchemaInferenceConfig,
) -> bool {
    let Some(name) = field_name.filter(|name| {
        config.force_scalar_promotion.contains(*name)
            && !config.force_field_types.contains_key(*name)
    }) else {
        return false;
    };
    let scalar_type = match schema.get("type") {
        Some(Value::String(t)) => Some(t.as_str()),
        Some(Value::Array(types)) if types.len() == 2 && types.iter().any(|t| t == "null") => {
            types.iter().find(|t| *t != "null").and_then(|t| t.as_str())
        }
        _ => None,
    };
    let Some(scalar_type) = scalar_type
        .filter(|t| matches!(*t, "string" | "integer" | "number" | "boolean"))
        .map(str::to_string)
    else {
        return false;
    };
    debug!(
        config,
        "Force promoting scalar field '{}' of type '{}'", name, scalar_type
    );
    let scalar = std::mem::take(schema);
    *schema = serde_json::json!({
        "type": "object",
        "properties": { make_promoted_scalar_key(name, &scalar_type): scalar }
    });
    true
}

/// A map's value schema when every field holds the same record, and there are several.
fn identical_record_values(props: &serde_json::Map<String, Value>) -> Option<Value> {
    let mut values = props.values();
    let first = values.next()?;
    let is_record = first.get("type") == Some(&Value::String("object".into()))
        && first.get("properties").is_some();
    (is_record && props.len() > 1 && values.collect::<Vec<_>>().par_iter().all(|v| *v == first))
        .then(|| first.clone())
}

/// A map's value schema when the fields' schemas are alike once nullability is set aside,
/// or else (with `unify_maps`) unify, its `anyOf` unions then processed.
fn unified_values(
    props: &serde_json::Map<String, Value>,
    field_name: Option<&str>,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let first = non_null_view(props.values().next()?);
    let views: Vec<NonNullView> = props.values().map(non_null_view).collect();
    if config.debug {
        debug_unique_schemas(&views, field_name, config);
    }
    let homog_start = std::time::Instant::now();
    let mut unified = if views.par_iter().all(|view| view.eq(&first)) {
        debug!(config, "Schemas are homogeneous after normalisation");
        Some(first.to_value())
    } else if !config.unify_maps {
        None
    } else if let Some(k) = props.keys().find(|k| config.no_unify.contains(k.as_str())) {
        debug!(config, "Not unifying: field {:?} is in no_unify", k);
        None
    } else {
        debug!(config, "Schemas not homogeneous, attempting unification");
        let children: Vec<&Value> = props.values().collect();
        let path = field_name.unwrap_or("");
        let unify_start = std::time::Instant::now();
        let unified = if children
            .par_iter()
            .all(|s| s.get("type") == Some(&Value::String("array".into())))
        {
            // Arrays: their items are unified, and every array must have them
            let items: Option<Vec<&Value>> = children.iter().map(|s| s.get("items")).collect();
            items
                .and_then(|items| check_unifiable_item_schemas(&items, path, config))
                .map(|items| serde_json::json!({ "type": "array", "items": items }))
        } else {
            check_unifiable_schemas(&children, path, config)
        };
        if config.profile && children.len() > 50 {
            anstream::eprintln!(
                "Unification of {} child schemas took {:?}",
                children.len(),
                unify_start.elapsed()
            );
        }
        unified
    };
    if config.profile && views.len() > 50 {
        anstream::eprintln!(
            "Homogeneity check on {} schemas took {:?}",
            views.len(),
            homog_start.elapsed()
        );
    }
    if let Some(schema) = unified.as_mut().filter(|schema| contains_anyof(schema)) {
        debug!(
            config,
            "Unified schema contains anyOf, processing unions first"
        );
        process_anyof_unions(schema, field_name.unwrap_or(""), config);
    }
    unified
}

/// Print the distinct schemas (nullability set aside) of a map candidate's fields.
fn debug_unique_schemas(
    views: &[NonNullView],
    field_name: Option<&str>,
    config: &SchemaInferenceConfig,
) {
    let unique: std::collections::BTreeSet<String> = views
        .iter()
        .map(|view| serde_json::to_string(&view.to_value()).unwrap_or_default())
        .collect();
    debug!(
        config,
        "Checking homogeneity for field {:?}: {} schemas, {} distinct",
        field_name.unwrap_or("root"),
        views.len(),
        unique.len()
    );
    if unique.len() <= 3 {
        for (i, schema) in unique.iter().enumerate() {
            debug!(config, "  Schema {}: {}", i, schema);
        }
    }
}
