// genson-core/src/schema/unification.rs
use crate::{debug, debug_verbose, schema::core::SchemaInferenceConfig};
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

/// Return a string representation of a JSON Schema type.
/// If it’s a union, pick the first non-"null" type.
fn schema_type_str(schema: &Value) -> String {
    if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
        return t.to_string();
    }

    // handle union case: ["null", {"type": "string"}]
    if let Some(arr) = schema.as_array() {
        for v in arr {
            if v != "null" {
                if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
                    return t.to_string();
                }
            }
        }
    }

    "unknown".to_string()
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
    if !schemas
        .iter()
        .all(|s| s.get("type") == Some(&Value::String("object".into())))
    {
        // debug!(config, "{path}: failed (non-object schema): {schemas:?}");
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

                        // Normalise before storing
                        e.insert(normalise_nullable(field_schema).clone());
                    }
                    ordermap::map::Entry::Occupied(mut e) => {
                        // Normalise both sides before comparison
                        let existing = normalise_nullable(e.get()).clone();
                        let new = normalise_nullable(field_schema).clone();

                        // First try the compatibility check for nullable/non-nullable
                        if let Some(compatible_schema) = schemas_compatible(&existing, &new) {
                            debug_verbose!(config, "Field `{field_name}` compatible (nullable/non-nullable unification)");
                            e.insert(compatible_schema);
                        } else if existing.get("type") == Some(&Value::String("object".into()))
                            && new.get("type") == Some(&Value::String("object".into()))
                        {
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
                        } else {
                            // Handle scalar vs object promotion if wrap_scalars is enabled
                            if config.wrap_scalars {
                                let existing_is_obj =
                                    existing.get("type") == Some(&Value::String("object".into()));
                                let new_is_obj = field_schema.get("type")
                                    == Some(&Value::String("object".into()));

                                if existing_is_obj ^ new_is_obj {
                                    // One is object, other is scalar → wrap scalar
                                    let (obj_schema, scalar_schema, scalar_side) =
                                        if existing_is_obj {
                                            (existing.clone(), field_schema.clone(), "new")
                                        } else {
                                            (field_schema.clone(), existing.clone(), "existing")
                                        };

                                    let type_suffix = schema_type_str(&scalar_schema);
                                    let wrapped_key = format!("{}__{}", field_name, type_suffix);

                                    debug!(config,
                                        "Promoting scalar on {} side: wrapping into object under key `{}`",
                                        scalar_side, wrapped_key
                                    );

                                    let mut wrapped_props = Map::new();
                                    wrapped_props.insert(wrapped_key, scalar_schema.clone());

                                    let promoted = json!({
                                        "type": "object",
                                        "properties": wrapped_props
                                    });

                                    // Recursively unify with the object schema
                                    if let Some(unified) = check_unifiable_schemas(
                                        &[obj_schema.clone(), promoted.clone()],
                                        &format!("{path}.{}", field_name),
                                        config,
                                    ) {
                                        debug!(config,
                                            "Field `{field_name}` unified successfully after scalar promotion"
                                        );
                                        e.insert(unified);
                                        continue;
                                    }
                                }
                            }

                            // If we didn’t handle it, it’s a true conflict
                            debug!(config,
                                "{path}.{field_name}: incompatible types:\n  existing={:#?}\n  new={:#?}",
                                existing, field_schema
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

    // Required in all -> non-nullable
    for (field_name, field_type) in &all_fields {
        let count = field_counts.get(field_name).unwrap_or(&0);
        if *count == total_schemas {
            debug_verbose!(
                config,
                "Field `{field_name}` present in all schemas → keeping non-nullable"
            );
            unified_properties.insert(field_name.clone(), field_type.clone());
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
                // Create a copy of the field_type and modify its type to be a union
                let mut nullable_field = field_type.clone();
                nullable_field["type"] = json!(["null", type_str]);
                unified_properties.insert(field_name.clone(), nullable_field);
            } else {
                // Fallback for schemas without explicit type
                unified_properties.insert(field_name.clone(), json!(["null", field_type]));
            }
        }
    }

    debug!(config, "Schemas unified successfully");
    Some(json!({
        "type": "object",
        "properties": unified_properties
    }))
}
