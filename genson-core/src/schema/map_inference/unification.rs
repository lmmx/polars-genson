// genson-core/src/schema/unification.rs
use crate::{
    debug, debug_verbose,
    schema::core::{make_promoted_scalar_key, SchemaInferenceConfig},
};
use rayon::prelude::*;
use serde_json::{json, Map, Value};
use std::borrow::Cow;

/// The scalar types: a schema of one of these unifies with another of the same type, and is
/// promoted (`wrap_scalars`) when it meets a record.
const SCALAR_TYPES: [&str; 4] = ["string", "number", "integer", "boolean"];

/// The schema inside any number of legacy nullable wrappers, `["null", <schema>]`.
fn normalise_nullable(mut schema: &Value) -> &Value {
    while let Value::Array(pair) = schema {
        match pair.as_slice() {
            [a, b] if a == "null" => schema = b,
            [a, b] if b == "null" => schema = a,
            _ => break,
        }
    }
    schema
}

/// The type a schema has besides null: `T` for `{"type": T}` or `{"type": ["null", T]}`.
fn base_type(schema: &Value) -> Option<&str> {
    match schema.get("type")? {
        Value::String(t) => Some(t),
        _ => nullable_type(schema)?.as_str(),
    }
}

/// The type of a scalar schema (`string`, `number`, `integer` or `boolean`, nullable or not).
fn get_scalar_type_name(schema: &Value) -> Option<&str> {
    base_type(schema).filter(|t| SCALAR_TYPES.contains(t))
}

/// Whether a schema is a scalar one, also inside legacy nullable wrappers.
fn is_scalar_schema(schema: &Value) -> bool {
    get_scalar_type_name(normalise_nullable(schema)).is_some()
}

/// Whether a schema is an array one, also inside legacy nullable wrappers.
fn is_array_schema(schema: &Value) -> bool {
    base_type(normalise_nullable(schema)) == Some("array")
}

/// Whether a schema is a record: an object with `properties`.
fn is_object_schema(schema: &Value) -> bool {
    base_type(schema) == Some("object") && schema.get("properties").is_some()
}

/// Whether a schema is a map: an object with `additionalProperties`.
fn is_map_schema(schema: &Value) -> bool {
    base_type(schema) == Some("object") && schema.get("additionalProperties").is_some()
}

/// Whether a schema is a record with no fields: an object whose `properties` are absent or
/// empty, and which is not a map (`additionalProperties` a schema or `true`).
fn is_empty_record_schema(schema: &Value) -> bool {
    if base_type(schema) != Some("object") {
        return false;
    }
    let values = schema.get("additionalProperties");
    if values.is_some_and(|v| v.is_object() || *v == Value::Bool(true)) {
        return false;
    }
    match schema.get("properties") {
        None => true,
        Some(properties) => properties.as_object().is_some_and(|p| p.is_empty()),
    }
}

/// For a nullable schema, `{"type": ["null", T], ...}`, its `T`.
fn nullable_type(schema: &Value) -> Option<&Value> {
    match schema.get("type")? {
        Value::Array(types) if types.len() == 2 && types.iter().any(|t| t == "null") => {
            types.iter().find(|t| *t != "null")
        }
        _ => None,
    }
}

/// The non-null schema of a nullable schema (`{"type": ["null", T]}` or the legacy
/// `["null", {...}]`), without copying it: a schema plus the `type` that replaces its own.
pub(super) struct NonNullView<'a> {
    schema: &'a Value,
    type_override: Option<&'a Value>,
}

pub(super) fn non_null_view(schema: &Value) -> NonNullView<'_> {
    if let Value::Array(arr) = schema {
        if arr.len() == 2 && arr.iter().any(|v| v == "null") {
            if let Some(inner) = arr.iter().find(|v| *v != "null") {
                return NonNullView {
                    schema: inner,
                    type_override: None,
                };
            }
        }
    }
    NonNullView {
        schema,
        type_override: nullable_type(schema),
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
    pub(super) fn eq(&self, other: &NonNullView) -> bool {
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

    pub(super) fn to_value(&self) -> Value {
        let mut schema = self.schema.clone();
        if let (Some(t), Some(obj)) = (self.type_override, schema.as_object_mut()) {
            obj.insert("type".to_string(), t.clone());
        }
        schema
    }
}

/// `b` made nullable, when `a` is the null schema and `b` has a type.
fn try_make_nullable_union(a: &Value, b: &Value) -> Option<Value> {
    if a.get("type")? != "null" {
        return None;
    }
    let other_type = b.get("type").filter(|t| *t != "null")?;
    let mut result = b.clone();
    result
        .as_object_mut()?
        .insert("type".to_string(), json!(["null", other_type]));
    Some(result)
}

/// The two schemas as one when they differ at most in nullability: equal, one of them
/// the null schema, or equal once `["null", T]` types are read as `T` (then nullable).
fn schemas_compatible(existing: &Value, new: &Value) -> Option<Value> {
    if existing == new {
        return Some(existing.clone());
    }
    if let Some(result) =
        try_make_nullable_union(existing, new).or_else(|| try_make_nullable_union(new, existing))
    {
        return Some(result);
    }
    // Read without the `null` in their types. Without it in either, they are unequal.
    let without_null = |schema| NonNullView {
        schema,
        type_override: nullable_type(schema),
    };
    let (existing_view, new_view) = (without_null(existing), without_null(new));
    if existing_view
        .type_override
        .or(new_view.type_override)
        .is_none()
        || !existing_view.eq(&new_view)
    {
        return None;
    }
    let mut nullable = existing_view.to_value();
    if let Some(inner_type) = nullable.get("type").cloned() {
        nullable["type"] = json!(["null", inner_type]);
    }
    Some(nullable)
}

/// The field name to promote scalars under when unifying the schemas at `path`: its
/// innermost field, or `value` where there is none (as for the values of a map, which is
/// where the kv map encoding puts them)
fn promotion_field_name(path: &str) -> &str {
    path.rsplit('.')
        .find(|s| !s.is_empty() && *s != "items" && *s != "additionalProperties")
        .unwrap_or("value")
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

    let wrapped_key = make_promoted_scalar_key(field_name, scalar_type);

    debug!(
        config,
        "Promoting scalar on {} side: wrapping {} into object under key `{}`",
        scalar_side,
        scalar_type,
        wrapped_key
    );

    let promoted = json!({
        "type": "object",
        "properties": { wrapped_key: scalar_schema }
    });

    // Recursively unify with the object schema
    let mut result = check_unifiable_schemas(
        &[object_schema, &promoted],
        &format!("{path}.{field_name}"),
        config,
    )?;

    // CRITICAL: Remove required array since all fields must be optional after scalar promotion
    if let Some(obj) = result.as_object_mut() {
        obj.shift_remove("required");
    }

    Some(result)
}

/// A keyword of a schema, also inside legacy nullable wrappers.
fn extract_field_from_nullable_schema<'a>(
    schema: &'a Value,
    field_name: &str,
) -> Option<&'a Value> {
    normalise_nullable(schema).get(field_name)
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

    // Extract all items schemas. An array seen only empty has no `items` or empty ones
    // (`{}`) and constrains nothing, so it unifies with any other array
    let mut items_schemas = Vec::<&Value>::new();
    for (i, &schema) in schemas.iter().enumerate() {
        let items = extract_field_from_nullable_schema(schema, "items")
            .filter(|items| items.as_object().is_none_or(|o| !o.is_empty()));
        if let Some(items) = items {
            debug_verbose!(
                config,
                "{}: Array schema[{}] items: {}",
                path,
                i,
                serde_json::to_string(items).unwrap_or_default()
            );
            items_schemas.push(items);
        } else {
            debug!(config, "{}: Array schema[{}] has no items (empty)", path, i);
        }
    }
    match items_schemas.as_slice() {
        [] => return Some(json!({ "type": "array" })),
        [only] => return Some(json!({ "type": "array", "items": only })),
        _ => {}
    }

    // Recursively unify the items
    if let Some(unified_items) =
        check_unifiable_item_schemas(&items_schemas, &format!("{}.items", path), config)
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

/// Unify scalar schemas of one type (nullable or not) into that type, nullable.
fn unify_scalar_schemas(
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.is_empty() {
        debug!(config, "Empty schema at {}", path);
        return None;
    }
    let base_types: std::collections::BTreeSet<&str> = schemas
        .iter()
        .filter_map(|&schema| base_type(schema))
        .collect();
    if let (1, Some(base_type)) = (base_types.len(), base_types.first()) {
        debug!(
            config,
            "{}: Unified scalar schemas to nullable {}", path, base_type
        );
        return Some(json!({"type": ["null", base_type]}));
    }
    debug!(
        config,
        "{}: Cannot unify incompatible scalar types: {:?}", path, base_types
    );
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

    let Some(additional_props_schemas) = schemas
        .iter()
        .map(|&schema| extract_field_from_nullable_schema(schema, "additionalProperties"))
        .collect::<Option<Vec<&Value>>>()
    else {
        debug!(
            config,
            "{}: A map schema is missing additionalProperties", path
        );
        return None;
    };

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

/// Unify one field's schemas pairwise in order, promoting scalars that meet records (or
/// other scalars) when `wrap_scalars` is set.
fn unify_field_schemas_sequential(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let (&first, rest) = schemas.split_first()?;
    if rest.iter().all(|&s| s == first) {
        return Some(first.clone());
    }
    let field_path = format!("{path}.{field_name}");
    let mut unified = first.clone();
    for &new in rest {
        unified = unify_field_pair(unified, new, field_name, &field_path, path, config)?;
    }
    Some(unified)
}

/// One step of `unify_field_schemas_sequential`: the field's schema so far with the next.
fn unify_field_pair(
    unified: Value,
    new: &Value,
    field_name: &str,
    field_path: &str,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if let Some(compatible) = schemas_compatible(&unified, new) {
        return Some(compatible);
    }
    let record_like = |s: &Value| is_object_schema(s) || is_empty_record_schema(s);
    if (is_array_schema(&unified) && is_array_schema(new))
        || (record_like(&unified) && record_like(new))
    {
        return check_unifiable_schemas(&[&unified, new], field_path, config);
    }
    if !config.wrap_scalars {
        return None;
    }
    if is_object_schema(&unified) && is_scalar_schema(new) {
        try_scalar_promotion(&unified, new, field_name, "new", path, config)
    } else if is_object_schema(new) && is_scalar_schema(&unified) {
        try_scalar_promotion(new, &unified, field_name, "existing", path, config)
    } else if is_scalar_schema(&unified) && is_scalar_schema(new) {
        try_mixed_scalar_promotion(&unified, new, field_name, path, config)
    } else {
        None
    }
}

/// Unify one field's schemas by halves in parallel (no scalar promotion across the halves).
fn unify_field_schemas_parallel(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if schemas.len() < 10 {
        return unify_field_schemas_sequential(field_name, schemas, path, config);
    }
    let (left, right) = schemas.split_at(schemas.len() / 2);
    let (left, right) = rayon::join(
        || unify_field_schemas_parallel(field_name, left, path, config),
        || unify_field_schemas_parallel(field_name, right, path, config),
    );
    check_unifiable_schemas(&[&left?, &right?], &format!("{path}.{field_name}"), config)
}

/// Unify one field's schemas: pairwise in order when there are few of them or scalars may
/// need promoting among records, otherwise by halves in parallel.
fn unify_field_schemas(
    field_name: &str,
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let needs_scalar_promo = config.wrap_scalars
        && schemas.iter().any(|&s| is_scalar_schema(s))
        && schemas.iter().any(|&s| is_object_schema(s));
    if needs_scalar_promo || schemas.len() < 50 {
        unify_field_schemas_sequential(field_name, schemas, path, config)
    } else {
        unify_field_schemas_parallel(field_name, schemas, path, config)
    }
}

/// Unify record schemas by merging their properties: each field's schemas are unified, a
/// field in every record stays as it is (and required), and one missing from some becomes
/// nullable.
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

    // Each field's schemas, in order of first appearance, and how many records hold it. A
    // field schema that is an `anyOf` goes in unified (by scalar promotion) after all the
    // others, so a field seen only as one comes after the rest.
    let mut fields: ordermap::OrderMap<&str, Vec<&Value>> = ordermap::OrderMap::new();
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut unified_anyofs: Vec<(&str, Value)> = Vec::new();
    for (i, &schema) in schemas.iter().enumerate() {
        let properties = match extract_field_from_nullable_schema(schema, "properties") {
            Some(Value::Object(properties)) => Some(properties),
            _ if is_empty_record_schema(schema) => None,
            _ => {
                debug!(config, "Schema[{i}] has no properties object");
                return None;
            }
        };
        for (name, field_schema) in properties.into_iter().flatten() {
            *counts.entry(name.as_str()).or_default() += 1;
            let field_schema = normalise_nullable(field_schema);
            if let Some(Value::Array(any_of)) = field_schema.get("anyOf") {
                let any_of: Vec<&Value> = any_of.iter().collect();
                if let Some(unified) = unify_anyof_schemas(&any_of, name, config) {
                    unified_anyofs.push((name.as_str(), unified));
                    continue;
                }
            }
            fields.entry(name.as_str()).or_default().push(field_schema);
        }
    }
    for (name, unified) in &unified_anyofs {
        fields.entry(name).or_default().push(unified);
    }

    let merge_start = std::time::Instant::now();
    let unify = |name: &'_ str, field_schemas: &[&Value]| {
        unify_field_schemas(name, field_schemas, path, config)
    };
    let unified: Option<Vec<Value>> = if fields.len() >= 10 {
        fields.par_iter().map(|(name, s)| unify(name, s)).collect()
    } else {
        fields.iter().map(|(name, s)| unify(name, s)).collect()
    };
    let Some(unified) = unified else {
        debug!(config, "Failed to unify field schemas");
        return None;
    };
    if config.profile && schemas.len() > 50 {
        anstream::eprintln!("  Merge loop took {:?}", merge_start.elapsed());
    }

    // Fields in every record first, as they are and required; then the rest, nullable
    let (in_all, in_some): (Vec<_>, Vec<_>) = fields
        .keys()
        .zip(unified)
        .map(|(&name, field_schema)| (name, counts[name], field_schema))
        .partition(|(_, count, _)| *count == schemas.len());
    let required: Vec<&str> = in_all.iter().map(|(name, _, _)| *name).collect();
    let mut properties = Map::new();
    for (name, _, field_schema) in in_all {
        debug_verbose!(
            config,
            "Field `{name}` present in all schemas → keeping non-nullable"
        );
        properties.insert(name.to_string(), field_schema);
    }
    for (name, count, field_schema) in in_some {
        debug_verbose!(
            config,
            "Field `{name}` missing in {}/{} schemas → making nullable",
            schemas.len() - count,
            schemas.len()
        );
        properties.insert(name.to_string(), make_nullable(field_schema));
    }

    debug!(config, "{}: Record schemas unified successfully", path);
    let mut result = json!({
        "type": "object",
        "properties": properties
    });
    if !required.is_empty() {
        result["required"] = json!(required);
    }
    Some(result)
}

/// A field schema made nullable: a type `T` becomes `["null", T]` (a nullable or null type
/// stays as it is), and a schema without a type becomes `anyOf` null and itself.
fn make_nullable(mut schema: Value) -> Value {
    match schema.get("type") {
        Some(Value::String(t)) if t != "null" => {
            let t = t.clone();
            schema["type"] = json!(["null", t]);
            schema
        }
        Some(Value::String(_) | Value::Array(_)) => schema,
        _ => json!({ "anyOf": [{ "type": "null" }, schema] }),
    }
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

    // No required array: each promoted field holds a value in only some rows
    Some(json!({
        "type": "object",
        "properties": {
            make_promoted_scalar_key(field_name, existing_type): existing,
            make_promoted_scalar_key(field_name, new_type): new,
        }
    }))
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

    // Each scalar wrapped as a record of one field, then all unified as records
    let promoted = schemas
        .iter()
        .map(|&schema| {
            if !is_scalar_schema(schema) {
                return Some(Cow::Borrowed(schema));
            }
            let wrapped_key = make_promoted_scalar_key(field_name, get_scalar_type_name(schema)?);
            Some(Cow::Owned(json!({
                "type": "object",
                "properties": { wrapped_key: schema }
            })))
        })
        .collect::<Option<Vec<Cow<Value>>>>()?;
    let promoted: Vec<&Value> = promoted.iter().map(Cow::as_ref).collect();
    check_unifiable_schemas(&promoted, field_name, config)
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

    // Check if all are map schemas OR empty records (semantically equivalent to empty maps)
    let all_maps_or_empty = schemas
        .iter()
        .all(|&s| is_map_schema(s) || is_empty_record_schema(s));

    if all_maps_or_empty {
        // Filter to only map schemas (ignore empty records)
        let map_schemas: Vec<&Value> = schemas
            .iter()
            .filter(|&&s| is_map_schema(s))
            .copied()
            .collect();

        if map_schemas.is_empty() {
            // All schemas are empty records - treat as an empty map
            debug!(
                config,
                "{}: All schemas are empty records, treating as empty map", path
            );
            return Some(json!({
                "type": "object",
                "additionalProperties": {"type": "string"}
            }));
        }
        // The empty records among them contribute nothing
        debug!(
            config,
            "{}: Unifying {} maps ({} empty records left out)",
            path,
            map_schemas.len(),
            schemas.len() - map_schemas.len()
        );
        return unify_map_schemas(&map_schemas, path, config);
    }

    // Check if all are record schemas (objects with properties) OR empty records
    if schemas
        .iter()
        .all(|&s| is_object_schema(s) || is_empty_record_schema(s))
    {
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

/// Check if the item schemas of several arrays can be unified: as `check_unifiable_schemas`,
/// but records mixed with scalars are also unified (when `wrap_scalars` is set) by promoting
/// the scalars. A list's items are all meant to be alike, so a scalar among records is a
/// record collapsed to a scalar; a record's fields can differ in kind, so this is not done
/// for them.
pub(crate) fn check_unifiable_item_schemas(
    schemas: &[&Value],
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    check_unifiable_schemas(schemas, path, config).or_else(|| {
        let records_and_scalars = schemas
            .iter()
            .all(|&s| is_scalar_schema(s) || is_object_schema(s) || is_empty_record_schema(s));
        if !(config.wrap_scalars && records_and_scalars) {
            return None;
        }
        debug!(config, "{}: Promoting scalar items among records", path);
        unify_anyof_schemas(schemas, promotion_field_name(path), config)
    })
}

#[cfg(test)]
mod tests {
    include!("../../tests/unification.rs");
}
