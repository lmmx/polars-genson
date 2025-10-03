// genson-core/src/schema/unification.rs
use crate::{
    debug, debug_verbose,
    schema::core::{make_promoted_scalar_key, SchemaInferenceConfig},
};
use rayon::prelude::*;
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

/// Try to make a nullable union from a null type and a typed schema
fn try_make_nullable_union(a: &Value, b: &Value) -> Option<Value> {
    if a.get("type") == Some(&Value::String("null".into())) {
        if let Some(other_type) = b.get("type") {
            if other_type != &Value::String("null".into()) {
                let mut result = b.clone();
                result
                    .as_object_mut()?
                    .insert("type".to_string(), json!(["null", other_type]));
                return Some(result);
            }
        }
    }
    None
}

/// Helper function to check if two schemas are compatible (handling nullable vs non-nullable)
fn schemas_compatible(existing: &Value, new: &Value) -> Option<Value> {
    if existing == new {
        return Some(existing.clone());
    }

    // Handle null vs non-null type: create union
    if let Some(result) =
        try_make_nullable_union(existing, new).or_else(|| try_make_nullable_union(new, existing))
    {
        return Some(result);
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
        &[&object_schema.clone(), &promoted],
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
    schemas: &[&Value],
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
    let mut items_schemas = Vec::<&Value>::new();
    for (i, &schema) in schemas.iter().enumerate() {
        if let Some(items) = extract_field_from_nullable_schema(schema, "items") {
            debug_verbose!(
                config,
                "{}: Array schema[{}] items: {}",
                path,
                i,
                serde_json::to_string(items).unwrap_or_default()
            );
            items_schemas.push(items);
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
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.is_empty() {
        debug!(config, "Empty schema at {}", path);
        return None;
    }

    // Extract all the scalar types
    let mut base_types = std::collections::HashSet::new();

    for &schema in schemas {
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
    schemas: &[&Value],
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
    let mut additional_props_schemas = Vec::<&Value>::new();
    for (i, &schema) in schemas.iter().enumerate() {
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
            additional_props_schemas.push(additional_props);
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

/// Sequential pairwise unification with full scalar promotion support
fn unify_field_schemas_sequential(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> (String, Option<Value>) {
    if schemas.len() == 1 {
        return (field_name.to_string(), Some(schemas[0].clone()));
    }

    let first = schemas[0];
    if schemas.iter().all(|&s| s == first) {
        return (field_name.to_string(), Some(first.clone()));
    }

    let mut unified = schemas[0].clone();

    for &new in &schemas[1..] {
        if let Some(compatible) = schemas_compatible(&unified, new) {
            unified = compatible;
            continue;
        }

        if (is_array_schema(&unified) && is_array_schema(new))
            || (is_object_schema(&unified) && is_object_schema(new))
        {
            if let Some(result) = check_unifiable_schemas(
                &[&unified, new],
                &format!("{}.{}", path, field_name),
                config,
            ) {
                unified = result;
                continue;
            } else {
                return (field_name.to_string(), None);
            }
        }

        if config.wrap_scalars {
            let unified_is_obj = is_object_schema(&unified);
            let unified_is_scalar = is_scalar_schema(&unified);
            let new_is_obj = is_object_schema(new);
            let new_is_scalar = is_scalar_schema(new);

            if unified_is_obj && new_is_scalar {
                if let Some(result) =
                    try_scalar_promotion(&unified, new, field_name, "new", path, config)
                {
                    unified = result;
                    continue;
                }
            } else if new_is_obj && unified_is_scalar {
                if let Some(result) =
                    try_scalar_promotion(new, &unified, field_name, "existing", path, config)
                {
                    unified = result;
                    continue;
                }
            } else if unified_is_scalar && new_is_scalar {
                if let Some(result) =
                    try_mixed_scalar_promotion(&unified, new, field_name, path, config)
                {
                    unified = result;
                    continue;
                }
            }
        }

        return (field_name.to_string(), None);
    }

    (field_name.to_string(), Some(unified))
}

/// Divide-and-conquer parallel unification (no scalar promotion)
fn unify_field_schemas_parallel(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> (String, Option<Value>) {
    if schemas.is_empty() {
        return (field_name.to_string(), None);
    }
    if schemas.len() == 1 {
        return (field_name.to_string(), Some(schemas[0].clone()));
    }
    if schemas.len() < 10 {
        // Small sets: use sequential to avoid overhead
        return unify_field_schemas_sequential(field_name, schemas, path, config);
    }

    // Split and recurse in parallel
    let mid = schemas.len() / 2;
    let (left, right) = schemas.split_at(mid);

    let ((_l_name, l_res), (_r_name, r_res)) = rayon::join(
        || unify_field_schemas_parallel(field_name, left, path, config),
        || unify_field_schemas_parallel(field_name, right, path, config),
    );

    // Merge the two halves
    let merged = match (l_res, r_res) {
        (Some(lv), Some(rv)) => {
            check_unifiable_schemas(&[&lv, &rv], &format!("{}.{}", path, field_name), config)
        }
        _ => None,
    };

    (field_name.to_string(), merged)
}

/// Main entry point: choose strategy based on field characteristics
fn unify_field_schemas(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> (String, Option<Value>) {
    if schemas.len() == 1 {
        return (field_name.to_string(), Some(schemas[0].clone()));
    }

    // Check if we need scalar promotion for this field
    let needs_scalar_promo = config.wrap_scalars && {
        let has_scalars = schemas.iter().any(|&s| is_scalar_schema(s));
        let has_objects = schemas.iter().any(|&s| is_object_schema(s));
        has_scalars && has_objects
    };

    if needs_scalar_promo || schemas.len() < 50 {
        // Use sequential (preserves scalar promotion, good for small/mixed sets)
        unify_field_schemas_sequential(field_name, schemas, path, config)
    } else {
        // Use parallel divide-and-conquer (faster for large homogeneous sets)
        unify_field_schemas_parallel(field_name, schemas, path, config)
    }
}

/// Unify record schemas by merging their properties
fn unify_record_schemas(
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug!(
        config,
        "{}: Attempting to unify {} record schemas",
        path,
        schemas.len()
    );

    // Step 1: Extract all properties from all schemas IN PARALLEL (if large enough)
    let schema_properties: Vec<Option<serde_json::Map<String, Value>>> = if schemas.len() >= 50 {
        schemas
            .par_iter()
            .map(|&schema| {
                extract_field_from_nullable_schema(schema, "properties")
                    .and_then(|v| v.as_object())
                    .cloned()
            })
            .collect()
    } else {
        schemas
            .iter()
            .map(|&schema| {
                extract_field_from_nullable_schema(schema, "properties")
                    .and_then(|v| v.as_object())
                    .cloned()
            })
            .collect()
    };

    // Step 2: Collect all schemas for each field
    let mut field_schemas: ordermap::OrderMap<String, Vec<&Value>> = ordermap::OrderMap::new();
    let mut field_counts = ordermap::OrderMap::new();
    let mut unified_anyof_values: Vec<Value> = Vec::new();
    // Track which fields need unified anyOf values
    let mut anyof_indices: Vec<(String, usize)> = Vec::new();

    for (i, props_opt) in schema_properties.iter().enumerate() {
        let Some(props) = props_opt else {
            debug!(config, "Schema[{i}] has no properties object");
            return None;
        };

        for (field_name, field_schema) in props {
            *field_counts.entry(field_name.clone()).or_insert(0) += 1;

            let normalized = normalise_nullable(field_schema);

            // Handle anyOf before storing
            if let Some(Value::Array(any_of_schemas)) = normalized.get("anyOf") {
                let any_of_refs: Vec<&Value> = any_of_schemas.iter().collect();
                if let Some(unified) = unify_anyof_schemas(&any_of_refs, field_name, config) {
                    let idx = unified_anyof_values.len();
                    unified_anyof_values.push(unified);
                    anyof_indices.push((field_name.clone(), idx));
                    continue; // Skip the normal push
                }
            }

            // Normal path: store the normalized reference
            field_schemas
                .entry(field_name.clone())
                .or_default()
                .push(normalized);
        }
    }

    // Now add all the unified anyOf references
    for (field_name, idx) in anyof_indices {
        field_schemas
            .entry(field_name)
            .or_default()
            .push(&unified_anyof_values[idx]);
    }

    // Step 3: Unify schemas for each field
    let merge_start = std::time::Instant::now();
    let field_names: Vec<_> = field_schemas.keys().cloned().collect();

    let unified_fields: Vec<(String, Option<Value>)> = if field_names.len() >= 10 {
        field_names
            .par_iter()
            .map(|field_name| {
                unify_field_schemas(field_name, &field_schemas[field_name], path, config)
            })
            .collect()
    } else {
        field_names
            .iter()
            .map(|field_name| {
                unify_field_schemas(field_name, &field_schemas[field_name], path, config)
            })
            .collect()
    };

    // Build all_fields from results
    let mut all_fields = ordermap::OrderMap::new();
    for (field_name, unified_opt) in unified_fields {
        if let Some(unified) = unified_opt {
            all_fields.insert(field_name, unified);
        } else {
            debug!(config, "Failed to unify field schemas");
            return None;
        }
    }

    if config.profile && schemas.len() > 50 {
        eprintln!("  Merge loop took {:?}", merge_start.elapsed());
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
    schemas: &[&Value],
    field_name: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if !config.wrap_scalars {
        return None;
    }

    // Check if we have the specific case: some scalars, some objects
    let has_scalars = schemas.iter().any(|&s| is_scalar_schema(s));
    let has_objects = schemas.iter().any(|&s| is_object_schema(s));

    if !has_scalars || !has_objects {
        return None; // Not the mixed case we handle
    }

    debug!(
        config,
        "anyOf unification: promoting scalars for field '{}'", field_name
    );

    let mut promoted_schemas = Vec::new();

    for &schema in schemas {
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

    // Now unify the promoted schemas (all objects), after converting to references
    let promoted_refs: Vec<&Value> = promoted_schemas.iter().collect();
    check_unifiable_schemas(&promoted_refs, field_name, config)
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
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    debug_verbose!(
        config,
        "=== check_unifiable_schemas called with path='{}' and {} schemas:",
        path,
        schemas.len()
    );
    for (i, &schema) in schemas.iter().enumerate() {
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
    if schemas.iter().all(|&s| is_array_schema(s)) {
        debug!(
            config,
            "{}: All schemas are arrays, attempting array unification", path
        );
        return unify_array_schemas(schemas, path, config);
    }

    // Check if all are map schemas (objects with additionalProperties)
    if schemas.iter().all(|&s| is_map_schema(s)) {
        debug!(
            config,
            "{}: All schemas are maps, attempting map unification", path
        );
        return unify_map_schemas(schemas, path, config);
    }

    // Check if all are record schemas (objects with properties)
    if schemas.iter().all(|&s| is_object_schema(s)) {
        debug!(
            config,
            "{}: All schemas are records, attempting record unification", path
        );
        return unify_record_schemas(schemas, path, config);
    }

    // Check if all are scalar schemas
    if schemas.iter().all(|&s| is_scalar_schema(s)) {
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
    for (i, &schema) in schemas.iter().enumerate() {
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
