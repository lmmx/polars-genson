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
/// This ensures that equality checks and recursive unification don't
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
                if let Some(non_null_type) = type_arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                {
                    // Create a new schema with the non-null type, preserving other properties
                    let mut non_null_schema = schema.clone();
                    if let Some(obj) = non_null_schema.as_object_mut() {
                        obj.insert("type".to_string(), non_null_type.clone());
                        (true, non_null_schema)
                    } else {
                        (false, schema.clone())
                    }
                } else {
                    // Malformed nullable schema (e.g., ["null", "null"])
                    (false, schema.clone())
                }
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
                if let Some(obj) = nullable_schema.as_object_mut() {
                    obj.insert("type".to_string(), json!(["null", inner_type]));
                }
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

/// Check if a schema represents an object type (record with properties)
fn is_object_schema(schema: &Value) -> bool {
    // Check direct type field
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return type_str == "object" && schema.get("properties").is_some();
        }

        // Handle nullable format: {"type": ["null", "object"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                return non_null_type == Some("object") && schema.get("properties").is_some();
            }
        }
    }

    false
}

/// Check if a schema represents a map type (object with additionalProperties)
fn is_map_schema(schema: &Value) -> bool {
    // Check direct type field
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return type_str == "object" && schema.get("additionalProperties").is_some();
        }

        // Handle nullable format: {"type": ["null", "object"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                return non_null_type == Some("object")
                    && schema.get("additionalProperties").is_some();
            }
        }
    }

    false
}

/// Check if a schema represents an array type
fn is_array_schema(schema: &Value) -> bool {
    // Handle old legacy format first: ["null", {"type": "array"}]
    if let Value::Array(arr) = schema {
        if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
            let inner_schema = arr
                .iter()
                .find(|v| *v != &Value::String("null".to_string()))
                .unwrap();
            return is_array_schema(inner_schema); // Recursive call to handle nested nullability
        }
    }

    // Check direct type field
    if let Some(type_val) = schema.get("type") {
        if let Some(type_str) = type_val.as_str() {
            return type_str == "array";
        }

        // Handle nullable format: {"type": ["null", "array"]}
        if let Some(arr) = type_val.as_array() {
            if arr.len() == 2 && arr.contains(&Value::String("null".into())) {
                let non_null_type = arr
                    .iter()
                    .find(|t| *t != &Value::String("null".into()))
                    .and_then(|t| t.as_str());
                return non_null_type == Some("array");
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

/// Recursively unwrap nullable schema wrappers and extract a specific field.
///
/// Handles both legacy format `["null", {...}]` and modern format `{"type": ["null", "..."]}`.
/// Recursively unwraps multiple layers of nullable wrapping to find the inner schema,
/// then extracts the specified field from it.
fn extract_field_from_nullable_schema<'a>(
    schema: &'a Value,
    field_name: &str,
) -> Option<&'a Value> {
    // Handle legacy format: ["null", {...}]
    if let Value::Array(arr) = schema {
        if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
            let inner_schema = arr
                .iter()
                .find(|v| *v != &Value::String("null".to_string()))?;
            return extract_field_from_nullable_schema(inner_schema, field_name);
        }
    }

    // Handle modern nullable format: {"type": ["null", "array"]}
    if let Some(Value::Array(type_arr)) = schema.get("type") {
        if type_arr.len() == 2 && type_arr.contains(&Value::String("null".into())) {
            // For modern nullable, the field should be directly on this schema
            return schema.get(field_name);
        }
    }

    // Direct field extraction
    schema.get(field_name)
}

/// Unify array schemas by unifying their items
fn unify_array_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug!(
        config,
        "{}: Attempting to unify {} array schemas",
        path,
        schemas.len()
    );

    if schemas.is_empty() {
        return None;
    }

    // Extract all items schemas
    let mut items_schemas = Vec::new();
    for (i, schema) in schemas.iter().enumerate() {
        if let Some(items) = extract_field_from_nullable_schema(schema, "items") {
            debug_verbose!(
                config,
                "{}: Array schema[{}] items: {}",
                path,
                i,
                serde_json::to_string(items).unwrap_or_default()
            );
            items_schemas.push(items.clone());
        } else {
            debug!(config, "{}: Array schema[{}] missing items", path, i);
            return None;
        }
    }

    // Recursively unify the items
    if let Some(unified_items) =
        check_unifiable_schemas(&items_schemas, &format!("{}.items", path), config)
    {
        debug!(config, "{}: Successfully unified array items", path);
        Some(json!({
            "type": "array",
            "items": unified_items
        }))
    } else {
        debug!(config, "{}: Failed to unify array items", path);
        None
    }
}

fn unify_scalar_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.is_empty() {
        debug!(config, "Empty schema at {}", path);
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
    if config.debug {
        let mut sorted_types: Vec<_> = base_types.into_iter().collect();
        sorted_types.sort();
        debug!(
            config,
            "{}: Cannot unify incompatible scalar types: {:?}", path, sorted_types
        );
    }
    None
}

/// Unify map schemas by unifying their additionalProperties
fn unify_map_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug!(
        config,
        "{}: Attempting to unify {} map schemas",
        path,
        schemas.len()
    );

    if schemas.is_empty() {
        return None;
    }

    // Extract all additionalProperties schemas
    let mut additional_props_schemas = Vec::new();
    for (i, schema) in schemas.iter().enumerate() {
        if let Some(additional_props) =
            extract_field_from_nullable_schema(schema, "additionalProperties")
        {
            debug_verbose!(
                config,
                "{}: Map schema[{}] additionalProperties: {}",
                path,
                i,
                serde_json::to_string(additional_props).unwrap_or_default()
            );
            additional_props_schemas.push(additional_props.clone());
        } else {
            debug!(
                config,
                "{}: Map schema[{}] missing additionalProperties", path, i
            );
            return None;
        }
    }

    // Recursively unify the additionalProperties
    if let Some(unified_additional_props) = check_unifiable_schemas(
        &additional_props_schemas,
        &format!("{}.additionalProperties", path),
        config,
    ) {
        debug!(
            config,
            "{}: Successfully unified map additionalProperties", path
        );
        Some(json!({
            "type": "object",
            "additionalProperties": unified_additional_props
        }))
    } else {
        debug!(config, "{}: Failed to unify map additionalProperties", path);
        None
    }
}

/// Unify record schemas by merging their properties
fn unify_record_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug!(
        config,
        "{}: Attempting to unify {} record schemas",
        path,
        schemas.len()
    );

    let mut all_fields = ordermap::OrderMap::new();
    let mut field_counts = std::collections::HashMap::new();

    // Collect all field types and count occurrences
    for (i, schema) in schemas.iter().enumerate() {
        if let Some(Value::Object(props)) = extract_field_from_nullable_schema(schema, "properties")
        {
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
                        } else if is_array_schema(&existing) && is_array_schema(&new) {
                            // Try recursive unify if both are arrays
                            debug!(config,
                                "Field `{field_name}` has conflicting array schemas, attempting recursive unify"
                            );
                            if let Some(unified) = check_unifiable_schemas(
                                &[existing.clone(), new.clone()],
                                &format!("{path}.{}", field_name),
                                config,
                            ) {
                                debug!(
                                    config,
                                    "Field `{field_name}` unified successfully after array recursion"
                                );
                                e.insert(unified);
                            } else {
                                debug!(config, "{path}.{}: failed to unify arrays", field_name);
                                return None;
                            }
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
                            } else if existing_is_scalar && new_is_scalar {
                                // NEW: Handle scalar vs scalar conflicts with mixed types
                                debug!(config, "Field `{field_name}` has scalar vs scalar conflict, attempting promotion");

                                if let Some(unified) = try_mixed_scalar_promotion(
                                    &existing, &new, field_name, path, config,
                                ) {
                                    debug!(config, "Field `{field_name}` unified successfully after mixed scalar promotion");
                                    e.insert(unified);
                                    continue;
                                }
                            }

                            // If we reach here, it's not a valid promotion case
                            debug!(config,
                                "{path}.{field_name}: incompatible types (promotion failed):\n  existing={:#?}\n  new={:#?}",
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
                if type_str == "null" {
                    // Already null - don't double-wrap
                    unified_properties.insert(field_name.clone(), field_type.clone());
                } else {
                    // Make non-null type nullable
                    let mut nullable_field = field_type.clone();
                    nullable_field["type"] = json!(["null", type_str]);
                    unified_properties.insert(field_name.clone(), nullable_field);
                }
            } else if let Some(_type_arr) = field_type.get("type").and_then(|t| t.as_array()) {
                // Already nullable - use as is
                unified_properties.insert(field_name.clone(), field_type.clone());
            } else {
                // Complex schema - create proper anyOf union
                let nullable_schema = json!({
                    "anyOf": [
                        {"type": "null"},
                        field_type
                    ]
                });
                unified_properties.insert(field_name.clone(), nullable_schema);
            }
        }
    }

    debug!(config, "{}: Record schemas unified successfully", path);

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

/// Handle mixed scalar promotion when the same field has different scalar types
fn try_mixed_scalar_promotion(
    existing: &Value,
    new: &Value,
    field_name: &str,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    // Get scalar types from both schemas
    let existing_type = get_scalar_type_name(existing)?;
    let new_type = get_scalar_type_name(new)?;

    // Only promote if they're different scalar types
    if existing_type == new_type {
        return None;
    }

    debug!(
        config,
        "{}: Promoting mixed scalars {} and {} for field '{}'",
        path,
        existing_type,
        new_type,
        field_name
    );

    // Create promoted schemas
    let existing_key = make_promoted_scalar_key(field_name, &existing_type);
    let new_key = make_promoted_scalar_key(field_name, &new_type);

    let mut properties = Map::new();
    properties.insert(existing_key.clone(), existing.clone());
    properties.insert(new_key.clone(), new.clone());

    let promoted = json!({
        "type": "object",
        "properties": properties
        // No required array - all promoted fields should be nullable
    });

    Some(promoted)
}

pub(crate) fn unify_anyof_schemas(
    schemas: &[Value],
    field_name: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if !config.wrap_scalars {
        return None;
    }

    // Check if we have the specific case: some scalars, some objects
    let has_scalars = schemas.iter().any(is_scalar_schema);
    let has_objects = schemas.iter().any(is_object_schema);

    if !has_scalars || !has_objects {
        return None; // Not the mixed case we handle
    }

    debug!(
        config,
        "anyOf unification: promoting scalars for field '{}'", field_name
    );

    let mut promoted_schemas = Vec::new();

    for schema in schemas {
        if is_scalar_schema(schema) {
            if let Some(scalar_type) = get_scalar_type_name(schema) {
                let wrapped_key = make_promoted_scalar_key(field_name, &scalar_type);
                let promoted = json!({
                    "type": "object",
                    "properties": {
                        wrapped_key: schema.clone()
                    }
                });
                promoted_schemas.push(promoted);
            } else {
                return None;
            }
        } else {
            promoted_schemas.push(schema.clone());
        }
    }

    // Now unify the promoted schemas (all objects)
    check_unifiable_schemas(&promoted_schemas, field_name, config)
}

/// Check if a collection of schemas can be unified into a single schema.
///
/// This function determines whether heterogeneous schemas are "unifiable" - meaning they
/// can be merged into a single schema. This enables map inference for cases where values
/// have compatible but non-identical structures.
///
/// Supports unifying:
/// 1. Record schemas (objects with `properties`) - fields become selectively nullable
/// 2. Map schemas (objects with `additionalProperties`) - by unifying the value schemas  
/// 3. Scalar schemas with the same base type - creates nullable version
///
/// When `wrap_scalars` is enabled, scalar types that collide with object types are promoted
/// to singleton objects under a synthetic key (e.g., `value__string`), allowing unification
/// to succeed instead of failing.
///
/// # Returns
///
/// - `Some(unified_schema)` if schemas can be unified
/// - `None` if schemas cannot be unified due to fundamental incompatibilities
pub(crate) fn check_unifiable_schemas(
    schemas: &[Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug_verbose!(
        config,
        "=== check_unifiable_schemas called with path='{}' and {} schemas:",
        path,
        schemas.len()
    );
    for (i, schema) in schemas.iter().enumerate() {
        debug_verbose!(
            config,
            "  Schema[{}]: {}",
            i,
            serde_json::to_string(schema).unwrap_or_default()
        );
    }

    if schemas.is_empty() {
        debug!(config, "{path}: failed (empty schema list)");
        return None;
    }

    // Check if all are array schemas
    if schemas.iter().all(is_array_schema) {
        debug!(
            config,
            "{}: All schemas are arrays, attempting array unification", path
        );
        return unify_array_schemas(schemas, path, config);
    }

    // Check if all are map schemas (objects with additionalProperties)
    if schemas.iter().all(is_map_schema) {
        debug!(
            config,
            "{}: All schemas are maps, attempting map unification", path
        );
        return unify_map_schemas(schemas, path, config);
    }

    // Check if all are record schemas (objects with properties)
    if schemas.iter().all(is_object_schema) {
        debug!(
            config,
            "{}: All schemas are records, attempting record unification", path
        );
        return unify_record_schemas(schemas, path, config);
    }

    // Check if all are scalar schemas
    if schemas.iter().all(is_scalar_schema) {
        debug!(
            config,
            "{}: All schemas are scalars, attempting scalar unification", path
        );
        return unify_scalar_schemas(schemas, path, config);
    }

    // Mixed types - not supported yet
    debug!(
        config,
        "{}: Mixed schema types not supported for unification", path
    );
    for (i, schema) in schemas.iter().enumerate() {
        let schema_type = if is_array_schema(schema) {
            "array"
        } else if is_map_schema(schema) {
            "map"
        } else if is_object_schema(schema) {
            "record"
        } else if is_scalar_schema(schema) {
            "scalar"
        } else {
            "unknown"
        };
        debug!(
            config,
            "  Schema[{}] type: {} - {}",
            i,
            schema_type,
            serde_json::to_string(schema).unwrap_or_default()
        );
    }

    None
}

#[cfg(test)]
mod tests {
    include!("../../tests/unification.rs");
}
