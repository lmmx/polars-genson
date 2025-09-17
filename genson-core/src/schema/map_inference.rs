// genson-core/src/schema/map_inference.rs
// include!("map_inference/unification.rs"); // provides: check_unifiable_schemas
use crate::schema::core::SchemaInferenceConfig;
use serde_json::Value;
mod unification;
use unification::*;

/// Post-process an inferred JSON Schema to rewrite certain object shapes as maps.
///
/// This mutates the schema in place, applying user overrides and heuristics.
///
/// # Rules
/// - If the current field name matches a `force_field_types` override, that wins
///   (`"map"` rewrites to `additionalProperties`, `"record"` leaves as-is).
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
) {
    if let Value::Object(obj) = schema {
        // --- Forced overrides by field name ---
        if let Some(name) = field_name {
            if let Some(forced) = config.force_field_types.get(name) {
                match forced.as_str() {
                    "map" => {
                        obj.remove("properties");
                        obj.remove("required");
                        obj.insert(
                            "additionalProperties".to_string(),
                            serde_json::json!({ "type": "string" }),
                        );
                        return; // no need to apply heuristics or recurse
                    }
                    "record" => {
                        if let Some(props) =
                            obj.get_mut("properties").and_then(|p| p.as_object_mut())
                        {
                            for (k, v) in props {
                                rewrite_objects(v, Some(k), config);
                            }
                        }
                        if let Some(items) = obj.get_mut("items") {
                            rewrite_objects(items, None, config);
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }

        // --- Heuristic rewrite ---
        if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
            let key_count = props.len(); // |UK| - total keys observed
            let above_threshold = key_count >= config.map_threshold;

            // Copy out child schema shapes
            let child_schemas: Vec<Value> = props.values().cloned().collect();

            // Detect map-of-records only if:
            // - all children are identical
            // - and that child is itself an object with "properties" (i.e. a proper record)
            if above_threshold {
                if let Some(first) = child_schemas.first() {
                    if first.get("type") == Some(&Value::String("object".into()))
                        && first.get("properties").is_some()
                        && child_schemas.len() > 1
                    {
                        let all_same = child_schemas.iter().all(|other| other == first);
                        if all_same {
                            obj.remove("properties");
                            obj.remove("required");
                            obj.insert("additionalProperties".to_string(), first.clone());
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
                if props.values().all(|schema| schema == first_schema) {
                    // Handle union types properly - extract the non-null type for additionalProperties
                    if let Value::Array(arr) = first_schema {
                        if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
                            // This is a nullable union - extract the non-null type
                            let non_null_type = arr
                                .iter()
                                .find(|v| *v != &Value::String("null".to_string()))
                                .unwrap();
                            unified_schema = Some(non_null_type.clone());
                        } else {
                            unified_schema = Some(first_schema.clone());
                        }
                    } else {
                        unified_schema = Some(first_schema.clone());
                    }
                } else if config.unify_maps {
                    // Detect if these are all arrays of records
                    if child_schemas
                        .iter()
                        .all(|s| s.get("type") == Some(&Value::String("array".into())))
                    {
                        // Collect item schemas, short-circuit if any missing
                        let mut item_schemas = Vec::with_capacity(child_schemas.len());
                        let mut all_items_ok = true;
                        for s in &child_schemas {
                            if let Some(items) = s.get("items") {
                                item_schemas.push(items.clone());
                            } else {
                                all_items_ok = false;
                                break;
                            }
                        }
                        if all_items_ok {
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
                        }
                    } else {
                        unified_schema = check_unifiable_schemas(
                            &child_schemas,
                            field_name.unwrap_or(""),
                            config,
                        );
                    }
                }
            }

            // Apply map inference logic
            let should_be_map = if above_threshold && unified_schema.is_some() {
                if let Some(max_required) = config.map_max_required_keys {
                    required_key_count <= max_required
                } else {
                    true
                }
            } else {
                false
            };

            if should_be_map {
                if let Some(schema) = unified_schema {
                    obj.remove("properties");
                    obj.remove("required");
                    obj.insert("type".to_string(), Value::String("object".to_string()));
                    obj.insert("additionalProperties".to_string(), schema);
                    return;
                }
            }
        }

        // --- Recurse into nested values ---
        if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
            for (k, v) in props {
                rewrite_objects(v, Some(k), config);
            }
        }
        if let Some(items) = obj.get_mut("items") {
            rewrite_objects(items, None, config);
        }
        for v in obj.values_mut() {
            rewrite_objects(v, None, config);
        }
    } else if let Value::Array(arr) = schema {
        for v in arr {
            rewrite_objects(v, None, config);
        }
    }
}
