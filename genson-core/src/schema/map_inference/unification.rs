// genson-core/src/schema/unification.rs
use crate::{
    debug, debug_verbose,
    schema::core::{make_promoted_scalar_key, SchemaInferenceConfig},
};
use serde_json::{json, Map, Value};

/// Normalize a schema that may be wrapped in one or more layers of
/// `["null", <type>]` union arrays.
///
/// During inference, schemas often get wrapped in a nullable-union
/// more than once (e.g. `["null", ["null", {"type": "string"}]]`).
/// This helper strips away *all* redundant layers of `["null", ...]`
/// until only the innermost non-null schema remains.
///
/// This ensures that equality checks and recursive unification don’t
/// spuriously fail due to extra layers of null-wrapping.
fn normalise_nullable(v: &Value) -> &Value {
    let mut current = v;
    loop {
        if let Some(arr) = current.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
                // peel off the non-null element
                current = arr
                    .iter()
                    .find(|x| *x != &Value::String("null".to_string()))
                    .unwrap();
                continue;
            }
        }
        return current;
    }
}

/// Helper function to check if two schemas are compatible (handling nullable vs non-nullable)
fn schemas_compatible(existing: &Value, new: &Value) -> Option<Value> {
    if existing == new {
        return Some(existing.clone());
    }

    // Handle new JSON Schema nullable format: {"type": ["null", "string"]}
    let extract_nullable_info = |schema: &Value| -> (bool, Value) {
        if let Some(Value::Array(type_arr)) = schema.get("type") {
            if type_arr.len() == 2 && type_arr.contains(&Value::String("null".into())) {
                let non_null_type = type_arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .unwrap();

                // Create a new schema with the non-null type, preserving other properties
                let mut non_null_schema = schema.clone();
                non_null_schema
                    .as_object_mut()
                    .unwrap()
                    .insert("type".to_string(), non_null_type.clone());
                (true, non_null_schema)
            } else {
                (false, schema.clone())
            }
        } else {
            (false, schema.clone())
        }
    };

    let (existing_nullable, existing_inner) = extract_nullable_info(existing);
    let (new_nullable, new_inner) = extract_nullable_info(new);

    // If the inner schemas match (including all properties), return the nullable version
    if existing_inner == new_inner {
        if existing_nullable || new_nullable {
            // Create the nullable version by taking the non-nullable schema and making the type nullable
            let mut nullable_schema = existing_inner.clone();
            if let Some(inner_type) = existing_inner.get("type") {
                nullable_schema
                    .as_object_mut()
                    .unwrap()
                    .insert("type".to_string(), json!(["null", inner_type]));
            }
            return Some(nullable_schema);
        } else {
            return Some(existing_inner);
        }
    }

    None
}

/// Check if a schema represents a scalar type (not an object or array)
fn is_scalar_schema(schema: &Value) -> bool {
    // Handle old legacy format first: ["null", {"type": "string"}]
    if let Value::Array(arr) = schema {
        if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
            let inner_schema = arr
                .iter()
                .find(|v| *v != &Value::String("null".to_string()))
                .unwrap();
            return is_scalar_schema(inner_schema); // Recursive call
        }
    }

    // Check direct type field
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return matches!(type_str, "string" | "number" | "integer" | "boolean");
        }

        // Handle nullable format: {"type": ["null", "string"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                return matches!(
                    non_null_type,
                    Some("string" | "number" | "integer" | "boolean")
                );
            }
        }
    }

    false
}

/// Check if a schema represents an object type
fn is_object_schema(schema: &Value) -> bool {
    // Check direct type field
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return type_str == "object";
        }

        // Handle nullable format: {"type": ["null", "object"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                return non_null_type == Some("object");
            }
        }
    }

    false
}

/// Extract the scalar type name from a schema
fn get_scalar_type_name(schema: &Value) -> Option<String> {
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            if matches!(type_str, "string" | "number" | "integer" | "boolean") {
                return Some(type_str.to_string());
            }
        }

        // Handle nullable format: {"type": ["null", "string"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                if matches!(
                    non_null_type,
                    Some("string" | "number" | "integer" | "boolean")
                ) {
                    return non_null_type.map(|s| s.to_string());
                }
            }
        }
    }

    None
}

/// Attempt to promote a scalar schema to an object by wrapping it under a synthetic field name
fn try_scalar_promotion(
    object_schema: &Value,
    scalar_schema: &Value,
    field_name: &str,
    scalar_side: &str,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let Some(scalar_type) = get_scalar_type_name(scalar_schema) else {
        debug!(config, "Cannot determine scalar type for promotion");
        return None;
    };

    let wrapped_key = make_promoted_scalar_key(field_name, &scalar_type);

    debug!(
        config,
        "Promoting scalar on {} side: wrapping {} into object under key `{}`",
        scalar_side,
        scalar_type,
        wrapped_key
    );

    let mut wrapped_props = Map::new();
    wrapped_props.insert(wrapped_key, scalar_schema.clone());

    let promoted = json!({
        "type": "object",
        "properties": wrapped_props
    });

    // Recursively unify with the object schema
    check_unifiable_schemas(
        &[object_schema.clone(), promoted],
        &format!("{path}.{}", field_name),
        config,
    )
}

fn unify_scalar_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.is_empty() {
        return None;
    }

    // Extract all the scalar types
    let mut base_types = std::collections::HashSet::new();

    for schema in schemas {
        if let Some(type_val) = schema.get("type") {
            if let Some(type_str) = type_val.as_str() {
                // Direct scalar type
                base_types.insert(type_str.to_string());
            } else if let Some(arr) = type_val.as_array() {
                // Nullable scalar: ["null", "string"]
                if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                    if let Some(non_null_type) = arr
                        .iter()
                        .find(|t| *t != &Value::String("null".into()))
                        .and_then(|t| t.as_str())
                    {
                        base_types.insert(non_null_type.to_string());
                    }
                }
            }
        }
    }

    // If all schemas have the same base type, create a nullable version
    if base_types.len() == 1 {
        let base_type = base_types.iter().next().unwrap();
        debug!(
            config,
            "{}: Unified scalar schemas to nullable {}", path, base_type
        );
        return Some(json!({"type": ["null", base_type]}));
    }

    // Multiple incompatible scalar types
    debug!(
        config,
        "{}: Cannot unify incompatible scalar types: {:?}", path, base_types
    );
    None
}

/// Check if a collection of record schemas can be unified into a single schema with selective nullable fields.
///
/// This function determines whether heterogeneous record schemas are "unifiable" - meaning they
/// can be merged into a single schema where only missing fields become nullable. This enables
/// map inference for cases where record values have compatible but non-identical structures.
///
/// Schemas are considered unifiable if:
/// 1. All schemas represent record types (`"type": "object"` with `"properties"`)
/// 2. Field names are either disjoint OR have identical types when they overlap
/// 3. No field has conflicting type definitions across schemas
///
/// Fields present in all schemas remain required, while fields missing from some schemas
/// become nullable unions (e.g., `["null", {"type": "string"}]`).
///
/// When `wrap_scalars` is enabled, scalar types that collide with object types are promoted
/// to singleton objects under a synthetic key (e.g., `value__string`), allowing unification
/// to succeed instead of failing.
///
/// # Returns
///
/// - `Some(unified_schema)` if schemas can be unified - contains all unique fields with selective nullability
/// - `None` if schemas cannot be unified due to:
///   - Non-record types in the collection
///   - Conflicting field types (same field name, different types)
///   - Empty schema collection
pub(crate) fn check_unifiable_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.is_empty() {
        debug!(config, "{path}: failed (empty schema list)");
        return None;
    }

    // Only unify record schemas
    if !schemas.iter().all(is_object_schema) {
        // Check if these are all scalar schemas that can be unified
        if schemas.iter().all(is_scalar_schema) {
            debug!(
                config,
                "{}: All schemas are scalars, attempting scalar unification", path
            );
            return unify_scalar_schemas(schemas, path, config);
        } else {
            debug!(config, "{}: Not all schemas are scalars", path);
            for (i, schema) in schemas.iter().enumerate() {
                if !is_scalar_schema(schema) {
                    debug!(
                        config,
                        "  Schema {} (NOT scalar): {}",
                        i,
                        serde_json::to_string(schema).unwrap_or_default()
                    );
                }
            }
        }
        return None;
    }

    let mut all_fields = ordermap::OrderMap::new();
    let mut field_counts = std::collections::HashMap::new();

    // Collect all field types and count occurrences
    for (i, schema) in schemas.iter().enumerate() {
        if let Some(Value::Object(props)) = schema.get("properties") {
            for (field_name, field_schema) in props {
                *field_counts.entry(field_name.clone()).or_insert(0) += 1;

                match all_fields.entry(field_name.clone()) {
                    ordermap::map::Entry::Vacant(e) => {
                        debug_verbose!(config, "Schema[{i}] introduces new field `{field_name}`");
                        e.insert(normalise_nullable(field_schema).clone());
                    }
                    ordermap::map::Entry::Occupied(mut e) => {
                        let existing = normalise_nullable(e.get()).clone();
                        let new = normalise_nullable(field_schema).clone();

                        // First try the compatibility check for nullable/non-nullable
                        if let Some(compatible_schema) = schemas_compatible(&existing, &new) {
                            debug_verbose!(config, "Field `{field_name}` compatible (nullable/non-nullable unification)");
                            e.insert(compatible_schema);
                        } else if is_object_schema(&existing) && is_object_schema(&new) {
                            // Try recursive unify if both are objects
                            debug!(config,
                                "Field `{field_name}` has conflicting object schemas, attempting recursive unify"
                            );
                            if let Some(unified) = check_unifiable_schemas(
                                &[existing.clone(), new.clone()],
                                &format!("{path}.{}", field_name),
                                config,
                            ) {
                                debug!(
                                    config,
                                    "Field `{field_name}` unified successfully after recursion"
                                );
                                e.insert(unified);
                            } else {
                                debug!(config, "{path}.{}: failed to unify", field_name);
                                return None;
                            }
                        } else if config.wrap_scalars {
                            // Try scalar promotion only if one is truly a scalar and the other is an object
                            let existing_is_obj = is_object_schema(&existing);
                            let existing_is_scalar = is_scalar_schema(&existing);
                            let new_is_obj = is_object_schema(&new);
                            let new_is_scalar = is_scalar_schema(&new);

                            if existing_is_obj && new_is_scalar {
                                if let Some(unified) = try_scalar_promotion(
                                    &existing, &new, field_name, "new", path, config,
                                ) {
                                    debug!(config, "Field `{field_name}` unified successfully after scalar promotion");
                                    e.insert(unified);
                                    continue;
                                }
                            } else if new_is_obj && existing_is_scalar {
                                if let Some(unified) = try_scalar_promotion(
                                    &new, &existing, field_name, "existing", path, config,
                                ) {
                                    debug!(config, "Field `{field_name}` unified successfully after scalar promotion");
                                    e.insert(unified);
                                    continue;
                                }
                            }

                            // If we reach here, it's not a valid scalar/object promotion case
                            debug!(config,
                                "{path}.{field_name}: incompatible types (not scalar/object promotion):\n  existing={:#?}\n  new={:#?}",
                                existing, new
                            );
                            return None;
                        } else {
                            // If we didn't handle it, it's a true conflict
                            debug!(config,
                                "{path}.{field_name}: incompatible types:\n  existing={:#?}\n  new={:#?}",
                                existing, new
                            );
                            return None; // fundamentally incompatible types
                        }
                    }
                }
            }
        } else {
            debug!(config, "Schema[{i}] has no properties object");
            return None;
        }
    }

    let total_schemas = schemas.len();
    let mut unified_properties = Map::new();
    let mut required_fields = Vec::new();

    // Required in all -> non-nullable AND add to required array
    for (field_name, field_type) in &all_fields {
        let count = field_counts.get(field_name).unwrap_or(&0);
        if *count == total_schemas {
            debug_verbose!(
                config,
                "Field `{field_name}` present in all schemas → keeping non-nullable"
            );
            unified_properties.insert(field_name.clone(), field_type.clone());
            required_fields.push(field_name.clone()); // Add to required array
        }
    }

    // Missing in some -> nullable
    for (field_name, field_type) in &all_fields {
        let count = field_counts.get(field_name).unwrap_or(&0);
        if *count < total_schemas {
            debug_verbose!(
                config,
                "Field `{field_name}` missing in {}/{} schemas → making nullable",
                total_schemas - count,
                total_schemas
            );

            // Create proper JSON Schema nullable syntax
            if let Some(type_str) = field_type.get("type").and_then(|t| t.as_str()) {
                let mut nullable_field = field_type.clone();
                nullable_field["type"] = json!(["null", type_str]);
                unified_properties.insert(field_name.clone(), nullable_field);
            } else {
                unified_properties.insert(field_name.clone(), json!(["null", field_type]));
            }
        }
    }

    debug!(config, "Schemas unified successfully");

    // Build the final schema with required fields
    let mut result = json!({
        "type": "object",
        "properties": unified_properties
    });

    // Only add required array if there are required fields
    if !required_fields.is_empty() {
        result["required"] = json!(required_fields);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    include!("../../tests/unification.rs");
}
