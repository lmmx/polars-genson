// genson-core/src/schema/unification.rs
use crate::{
    debug, debug_verbose,
    schema::core::{make_promoted_scalar_key, SchemaInferenceConfig},
    value::object,
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
        .insert("type".to_string(), null_and(other_type.clone()));
    Some(result)
}

/// A `type` with `null` among its types: `["null", T]` for a type `T`, and a list of types
/// with `null` put first in it (rather than the list itself made the second of two types).
fn null_and(t: Value) -> Value {
    match t {
        Value::Array(types) if types.iter().any(|t| t == "null") => Value::Array(types),
        Value::Array(types) => Value::Array(std::iter::once("null".into()).chain(types).collect()),
        t => json!(["null", t]),
    }
}

/// Two unequal schemas as one when they differ only in nullability: one of them the null
/// schema, or equal once `["null", T]` types are read as `T` (then nullable).
fn schemas_compatible_unequal(existing: &Value, new: &Value) -> Option<Value> {
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
        nullable["type"] = null_and(inner_type);
    }
    Some(nullable)
}

/// `schemas_compatible_unequal` with `existing` owned: the same schema, made from
/// `existing` itself where it is the one made nullable, or `existing` back when the two
/// differ in more than nullability.
fn schemas_compatible_owned(existing: Value, new: &Value) -> Result<Value, Value> {
    if let Some(result) = try_make_nullable_union(&existing, new) {
        return Ok(result);
    }
    let mut existing = existing;
    // `new` the null schema: `existing` made nullable
    if new.get("type").is_some_and(|t| t == "null") {
        if let Some(other_type) = existing.get("type").filter(|t| *t != "null").cloned() {
            existing["type"] = null_and(other_type);
            return Ok(existing);
        }
    }
    let (existing_type, new_type) = (nullable_type(&existing), nullable_type(new));
    let (existing_view, new_view) = (
        NonNullView {
            schema: &existing,
            type_override: existing_type,
        },
        NonNullView {
            schema: new,
            type_override: new_type,
        },
    );
    if existing_type.or(new_type).is_none() || !existing_view.eq(&new_view) {
        return Err(existing);
    }
    if let Some(inner_type) = existing_type.or_else(|| existing.get("type")).cloned() {
        existing["type"] = null_and(inner_type);
    }
    Ok(existing)
}

/// `normalise_nullable` of an owned schema.
fn normalise_nullable_owned(mut schema: Value) -> Value {
    loop {
        let inner = match &schema {
            Value::Array(pair) if pair.len() == 2 && pair[0] == "null" => 1,
            Value::Array(pair) if pair.len() == 2 && pair[1] == "null" => 0,
            _ => return schema,
        };
        let Value::Array(mut pair) = schema else {
            unreachable!()
        };
        schema = pair.swap_remove(inner);
    }
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

/// Whether a map's values are typed null: the map was only ever empty, or held nulls alone.
fn is_null_values(values: &Value) -> bool {
    values
        .as_object()
        .is_some_and(|o| o.len() == 1 && o.get("type") == Some(&Value::from("null")))
}

/// Two maps one of which has values typed null, or whose values differ only in being
/// nullable (as a map unified with one of null values is), unified as
/// `unify_map_schemas` unifies them (`null ⊔ T` is nullable `T`), nullable where either map
/// is. A record field's two maps are otherwise unified only where alike.
fn unify_maps_beside_null_values(
    a: &Value,
    b: &Value,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if !(is_map_schema(a) && is_map_schema(b)) {
        return None;
    }
    let values = |s| extract_field_from_nullable_schema(s, "additionalProperties");
    let (a_values, b_values) = (values(a)?, values(b)?);
    if !(is_null_values(a_values)
        || is_null_values(b_values)
        || non_null_view(a_values).eq(&non_null_view(b_values)))
    {
        return None;
    }
    let map = unify_map_schemas(&[a, b], path, config)?;
    Some(match nullable_type(a).or(nullable_type(b)) {
        Some(_) => make_nullable(map),
        None => map,
    })
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
        [only] => {
            return Some(object([
                ("type", "array".into()),
                ("items", (*only).clone()),
            ]))
        }
        _ => {}
    }

    // Recursively unify the items
    if let Some(unified_items) =
        check_unifiable_item_schemas(&items_schemas, &format!("{}.items", path), config)
    {
        debug!(config, "{}: Successfully unified array items", path);
        Some(object([("type", "array".into()), ("items", unified_items)]))
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

    // Values typed null (a map only ever empty, or holding nulls alone) join the others as
    // null does: `null ⊔ T` is `T` made nullable, and `null ⊔ null` is null
    let (nulls, typed): (Vec<&Value>, Vec<&Value>) = additional_props_schemas
        .iter()
        .partition(|v| is_null_values(v));
    let unified = match typed.as_slice() {
        [] => Some(json!({ "type": "null" })),
        _ => check_unifiable_schemas(&typed, &format!("{}.additionalProperties", path), config)
            .map(|values| match nulls.is_empty() {
                true => values,
                false => make_nullable(values),
            }),
    };

    // Recursively unify the additionalProperties
    if let Some(unified_additional_props) = unified {
        debug!(
            config,
            "{}: Successfully unified map additionalProperties", path
        );
        Some(object([
            ("type", "object".into()),
            ("additionalProperties", unified_additional_props),
        ]))
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
    let field_path = format!("{path}.{field_name}");
    // The first schema is copied only if no step changes it
    let mut unified = Cow::Borrowed(first);
    for &new in rest {
        // A schema equal to the one so far leaves it as it is
        if *unified == *new {
            continue;
        }
        unified = Cow::Owned(unify_field_pair(
            unified,
            new,
            field_name,
            &field_path,
            path,
            config,
        )?);
    }
    Some(unified.into_owned())
}

/// One step of `unify_field_schemas_sequential`: the field's schema so far with the next,
/// which differs from it. A schema so far that is owned has what it shares with the next
/// moved into the result rather than copied.
fn unify_field_pair(
    unified: Cow<Value>,
    new: &Value,
    field_name: &str,
    field_path: &str,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let unified = match unified {
        Cow::Owned(unified) if !config.debug => match schemas_compatible_owned(unified, new) {
            Ok(compatible) => return Some(compatible),
            Err(unified) => Cow::Owned(unified),
        },
        unified => match schemas_compatible_unequal(&unified, new) {
            Some(compatible) => return Some(compatible),
            None => unified,
        },
    };
    if let Some(map) = unify_maps_beside_null_values(&unified, new, field_path, config) {
        return Some(map);
    }
    let record_like = |s: &Value| is_object_schema(s) || is_empty_record_schema(s);
    if (is_array_schema(&unified) && is_array_schema(new))
        || (record_like(&unified) && record_like(new))
    {
        return match unified {
            Cow::Owned(unified) => unify_pair(unified, new, field_path, config),
            Cow::Borrowed(unified) => check_unifiable_schemas(&[unified, new], field_path, config),
        };
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
        // An array beside a record or scalar: one array holding them, as for a union
        unify_anyof_schemas(&[&unified, new], field_name, config)
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
    unify_pair(left?, &right?, &format!("{path}.{field_name}"), config)
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
        && ((schemas.iter().any(|&s| is_scalar_schema(s))
            && schemas.iter().any(|&s| is_object_schema(s)))
            || (schemas.iter().any(|&s| is_array_schema(s))
                && schemas.iter().any(|&s| !is_array_schema(s))));
    if needs_scalar_promo || schemas.len() < 50 {
        unify_field_schemas_sequential(field_name, schemas, path, config)
    } else {
        unify_field_schemas_parallel(field_name, schemas, path, config)
    }
}

/// Unify record schemas by merging their properties: each field's schemas are unified, a
/// field every record requires stays as it is (and required), and any other becomes
/// nullable. A field's presence in every record's `properties` does not make it required:
/// a record that is itself a unified schema lists every field seen so far, required or not.
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
    // How many records require each field
    let mut required_in: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut unified_anyofs: Vec<(&str, Value)> = Vec::new();
    for (i, &schema) in schemas.iter().enumerate() {
        if let Some(Value::Array(required)) = extract_field_from_nullable_schema(schema, "required")
        {
            for name in required.iter().filter_map(Value::as_str) {
                *required_in.entry(name).or_default() += 1;
            }
        }
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

    // Fields in every record first, then the rest; a field every record requires is kept
    // as it is and required, any other made nullable
    let (in_all, in_some): (Vec<_>, Vec<_>) = fields
        .keys()
        .zip(unified)
        .map(|(&name, field_schema)| (name, counts[name], field_schema))
        .partition(|(_, count, _)| *count == schemas.len());
    let required_by_all = |name: &str| required_in.get(name) == Some(&schemas.len());
    let required: Vec<&str> = in_all
        .iter()
        .map(|(name, _, _)| *name)
        .filter(|name| required_by_all(name))
        .collect();
    let mut properties = Map::new();
    for (name, _, field_schema) in in_all {
        if required_by_all(name) {
            debug_verbose!(
                config,
                "Field `{name}` required in all schemas → keeping non-nullable"
            );
            properties.insert(name.to_string(), field_schema);
        } else {
            debug_verbose!(
                config,
                "Field `{name}` optional in some schemas → making nullable"
            );
            properties.insert(name.to_string(), make_nullable(field_schema));
        }
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
    let mut result = object([
        ("type", "object".into()),
        ("properties", Value::Object(properties)),
    ]);
    if !required.is_empty() {
        result["required"] = json!(required);
    }
    Some(result)
}

/// A field schema made nullable: a type `T` becomes `["null", T]` (a nullable or null type
/// stays as it is), and a schema without a type becomes `anyOf` null and itself.
pub(crate) fn make_nullable(mut schema: Value) -> Value {
    match schema.get("type") {
        Some(Value::String(t)) if t != "null" => {
            let t = t.clone();
            schema["type"] = json!(["null", t]);
            schema
        }
        Some(Value::String(_) | Value::Array(_)) => schema,
        _ => object([(
            "anyOf",
            Value::Array(vec![json!({ "type": "null" }), schema]),
        )]),
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

/// Whether types (other than `null`) hold values no one of them can: scalars of two kinds
/// other than an integer and a number (which widen to a number), or a record or an array
/// beside another type. A string does not clash with numbers or booleans it is to be
/// coerced to (`coerce_strings`).
fn kinds_clash<'a>(
    kinds: impl IntoIterator<Item = &'a str>,
    config: &SchemaInferenceConfig,
) -> bool {
    let kinds: std::collections::BTreeSet<&str> = kinds.into_iter().collect();
    let coerced = config.coerce_strings
        && kinds
            .iter()
            .any(|&t| matches!(t, "integer" | "number" | "boolean"));
    let kinds: Vec<&str> = kinds
        .into_iter()
        .filter(|&t| t != "null" && !(coerced && t == "string"))
        .collect();
    kinds.len() >= 2 && !kinds.iter().all(|&t| t == "integer" || t == "number")
}

/// Whether a schema of several types holds values no one of them can (`kinds_clash`). Its
/// values are kept as an `anyOf` of its types would keep them.
pub(crate) fn type_union_clashes(
    schema: &Map<String, Value>,
    config: &SchemaInferenceConfig,
) -> bool {
    let Some(Value::Array(types)) = schema.get("type") else {
        return false;
    };
    kinds_clash(types.iter().filter_map(Value::as_str), config)
}

/// The branches of a schema of several types, one per type: `{"type": ["object",
/// "integer"]}` is a record with no fields and an integer, the record taking the schema's
/// `properties`, `required` and `additionalProperties`, and an array its `items`. In
/// `type_rank`'s order, so what they are rewritten to does not depend on the order their
/// types were seen. `None` for a schema of one type, nullable or not.
pub(crate) fn type_union_branches(schema: &Map<String, Value>) -> Option<Vec<Value>> {
    let Some(Value::Array(types)) = schema.get("type") else {
        return None;
    };
    let mut types: Vec<&str> = types.iter().map(Value::as_str).collect::<Option<_>>()?;
    if types.iter().filter(|&&t| t != "null").count() < 2 {
        return None;
    }
    types.sort_by_key(|t| crate::schema::type_string_rank(t));
    let branches = types.into_iter().map(|t| {
        let keys: &[&str] = match t {
            "object" => &["properties", "required", "additionalProperties"],
            "array" => &["items"],
            _ => &[],
        };
        let mut branch = Map::new();
        branch.insert("type".to_string(), t.into());
        for &key in keys {
            if let Some(value) = schema.get(key) {
                branch.insert(key.to_string(), value.clone());
            }
        }
        Value::Object(branch)
    });
    Some(branches.collect())
}

/// Whether a schema is `null` alone.
pub(crate) fn is_null_schema(schema: &Value) -> bool {
    schema.get("type").is_some_and(|t| t == "null")
}

/// Whether the scalars of these schemas are of kinds that clash (`kinds_clash`).
fn scalar_kinds_clash(schemas: &[&Value], config: &SchemaInferenceConfig) -> bool {
    let kinds = schemas
        .iter()
        .filter_map(|&s| get_scalar_type_name(normalise_nullable(s)));
    kinds_clash(kinds, config)
}

pub(crate) fn unify_anyof_schemas(
    schemas: &[&Value],
    field_name: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if !config.wrap_scalars {
        return None;
    }
    // A branch that is a union of its own (an `anyOf`, or several types that clash) stands
    // for its branches
    let is_union = |s: &&Value| {
        s.get("anyOf").is_some() || s.as_object().is_some_and(|o| type_union_clashes(o, config))
    };
    let split: Option<Vec<Value>> = schemas.iter().any(is_union).then(|| {
        let mut split = Vec::new();
        for &s in schemas {
            union_branches(s, config, &mut split);
        }
        split
    });
    let flat: Vec<&Value>;
    let schemas = match &split {
        Some(split) => {
            flat = split.iter().collect();
            flat.as_slice()
        }
        None => schemas,
    };
    if let Some(array) = unify_into_array(schemas, config) {
        return Some(array);
    }
    // `null` beside two or more schemas that unify: their unified schema, nullable
    let nullable = schemas.iter().any(|&s| is_null_schema(s));
    let non_null: Vec<&Value> = schemas
        .iter()
        .copied()
        .filter(|&s| !is_null_schema(s))
        .collect();
    if nullable && non_null.len() >= 2 {
        if let Some(unified) = check_unifiable_schemas(&non_null, field_name, config) {
            return Some(make_nullable(unified));
        }
    }

    // Scalars among records (also records with no fields), or scalars of kinds that do not
    // widen to one: each kind is kept in a field of its own
    let has_scalars = schemas.iter().any(|&s| is_scalar_schema(s));
    let has_objects = schemas
        .iter()
        .any(|&s| is_object_schema(s) || is_empty_record_schema(s));

    // Integers and numbers alone (beside `null`) widen to a number
    let number = |s: &Value| matches!(base_type(s), Some("integer" | "number"));
    if non_null.len() >= 2 && non_null.iter().all(|&s| number(s)) {
        let typed_null = |s: &Value| {
            s.get("type")
                .and_then(Value::as_array)
                .is_some_and(|t| t.iter().any(|t| t == "null"))
        };
        let widened = json!({ "type": "number" });
        return Some(if nullable || non_null.iter().any(|&s| typed_null(s)) {
            make_nullable(widened)
        } else {
            widened
        });
    }

    if !has_scalars || !(has_objects || scalar_kinds_clash(schemas, config)) {
        return None; // Not the mixed case we handle
    }

    debug!(
        config,
        "anyOf unification: promoting scalars for field '{}'", field_name
    );

    // Each scalar wrapped as a record of one field, then all unified as records; `null`
    // makes the record nullable (a promoted field holds a value in only some rows)
    let promoted = schemas
        .iter()
        .filter(|&&schema| !is_null_schema(schema))
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
    let unified = check_unifiable_schemas(&promoted, field_name, config)?;
    Some(if nullable {
        make_nullable(unified)
    } else {
        unified
    })
}

/// The branches a union stands for, all the way down: an `anyOf`'s branches, and a schema
/// of several types that clash its types' (`type_union_branches`).
fn union_branches(schema: &Value, config: &SchemaInferenceConfig, out: &mut Vec<Value>) {
    if let Some(Value::Array(any_of)) = schema.get("anyOf") {
        for branch in any_of {
            union_branches(branch, config, out);
        }
        return;
    }
    let branches = schema
        .as_object()
        .filter(|o| type_union_clashes(o, config))
        .and_then(type_union_branches);
    match branches {
        Some(branches) => out.extend(branches),
        None => out.push(schema.clone()),
    }
}

/// `unify_into_array` of a union's branches once each is rewritten: an array's items may
/// unify with the other branches only then.
pub(crate) fn unify_rewritten_into_array(
    branches: &[&Value],
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    if !config.wrap_scalars {
        return None;
    }
    unify_into_array(branches, config)
}

/// A union branch without `null`, and whether it held `null`: `None` for a branch that is
/// `null` alone.
fn without_null(schema: &Value) -> (Option<Cow<'_, Value>>, bool) {
    let inner = normalise_nullable(schema);
    let wrapped = !std::ptr::eq(inner, schema);
    match inner.get("type") {
        Some(t) if t == "null" => (None, true),
        Some(Value::Array(types)) if types.iter().any(|t| t == "null") => {
            let mut rest: Vec<Value> = types.iter().filter(|t| *t != "null").cloned().collect();
            let t = match rest.len() {
                0 => return (None, true),
                1 => rest.swap_remove(0),
                _ => Value::Array(rest),
            };
            let mut owned = inner.clone();
            owned["type"] = t;
            (Some(Cow::Owned(owned)), true)
        }
        _ => (Some(Cow::Borrowed(inner)), wrapped),
    }
}

/// A union of arrays and other schemas as one array, whose items unify the arrays' items
/// with the other schemas: normalisation wraps a value that is not an array into the array,
/// so the items must hold it. Scalars among records promote as they do among an array's
/// items, under no field name (`__string`). A union that holds `null` (as a branch, or in a
/// branch's types) stays one of the array and `null`. `None` without both an array and
/// another schema besides `null`, where a branch has other than one type besides `null`,
/// or where the items do not unify.
fn unify_into_array(schemas: &[&Value], config: &SchemaInferenceConfig) -> Option<Value> {
    let (mut arrays, mut others, mut nullable) = (Vec::new(), Vec::new(), false);
    for &schema in schemas {
        let (schema, null) = without_null(schema);
        nullable |= null;
        match schema {
            // A branch of one type: one of several, or none, is a union of its own
            Some(schema) if !schema.get("type").is_some_and(Value::is_string) => return None,
            Some(schema) if is_array_schema(&schema) => arrays.push(schema),
            Some(schema) => others.push(schema),
            None => {}
        }
    }
    if arrays.is_empty() || others.is_empty() {
        return None;
    }
    // An array seen only empty has no items, or empty ones, and constrains nothing
    // Items seen only as `null` type nothing but the items' nullability
    let mut null_items = false;
    let mut items: Vec<&Value> = arrays
        .iter()
        .filter_map(|a| a.get("items"))
        .filter(|items| items.as_object().is_none_or(|o| !o.is_empty()))
        .filter(|&items| {
            null_items |= is_null_schema(items);
            !is_null_schema(items)
        })
        .collect();
    items.extend(others.iter().map(Cow::as_ref));
    let items = match items.as_slice() {
        [only] => (*only).clone(),
        _ => check_unifiable_schemas(&items, "", config)
            .or_else(|| unify_anyof_schemas(&items, "", config))?,
    };
    let items = if null_items {
        make_nullable(items)
    } else {
        items
    };
    debug!(
        config,
        "anyOf unification: other branches wrapped into the array"
    );
    let array = object([("type", "array".into()), ("items", items)]);
    Some(if nullable {
        object([("anyOf", json!([array, {"type": "null"}]))])
    } else {
        array
    })
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
        // An array among other items: one array holding them, as for a union
        let arrays_and_others = schemas.iter().any(|&s| is_array_schema(s))
            && schemas.iter().any(|&s| !is_array_schema(s));
        if !(config.wrap_scalars && (records_and_scalars || arrays_and_others)) {
            return None;
        }
        debug!(config, "{}: Promoting scalar items among records", path);
        unify_anyof_schemas(schemas, promotion_field_name(path), config)
    })
}

/// `check_unifiable_schemas` of `[a, b]` with `a` owned: the same schema, with what `a`
/// shares with `b` moved into it rather than copied. Debugging takes the borrowed way, so
/// that it reports as it did.
fn unify_pair(a: Value, b: &Value, path: &str, config: &SchemaInferenceConfig) -> Option<Value> {
    if config.debug {
        return check_unifiable_schemas(&[&a, b], path, config);
    }
    if is_array_schema(&a) && is_array_schema(b) {
        return unify_array_pair(a, b, path, config);
    }
    if (is_map_schema(&a) || is_empty_record_schema(&a))
        && (is_map_schema(b) || is_empty_record_schema(b))
    {
        if !(is_map_schema(&a) && is_map_schema(b)) {
            return check_unifiable_schemas(&[&a, b], path, config);
        }
        // Two maps, one with values typed null: as `unify_map_schemas` unifies them
        let null_values = |s: &Value| s.get("additionalProperties").is_some_and(is_null_values);
        if null_values(&a) || null_values(b) {
            return check_unifiable_schemas(&[&a, b], path, config);
        }
        // Two maps: their values unified
        let Value::Object(mut a) = a else {
            unreachable!()
        };
        let a_values = a.swap_remove("additionalProperties")?;
        let b_values = b.get("additionalProperties")?;
        let values = unify_pair(
            a_values,
            b_values,
            &format!("{}.additionalProperties", path),
            config,
        )?;
        return Some(object([
            ("type", "object".into()),
            ("additionalProperties", values),
        ]));
    }
    if (is_object_schema(&a) || is_empty_record_schema(&a))
        && (is_object_schema(b) || is_empty_record_schema(b))
    {
        return unify_record_pair(a, b, path, config);
    }
    // Scalars, or kinds that do not unify: nothing of `a` to move
    check_unifiable_schemas(&[&a, b], path, config)
}

/// `unify_array_schemas` of `[a, b]` with `a` owned, as `unify_pair`.
fn unify_array_pair(
    a: Value,
    b: &Value,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    // An array in a legacy nullable wrapper has its items read from inside it, as there
    let Value::Object(mut a) = normalise_nullable_owned(a) else {
        unreachable!("an array schema is an object")
    };
    let has_items = |items: &Value| items.as_object().is_none_or(|o| !o.is_empty());
    let a_items = a.swap_remove("items").filter(has_items);
    let b_items = extract_field_from_nullable_schema(b, "items").filter(|items| has_items(items));
    let items = match (a_items, b_items) {
        (None, None) => return Some(json!({ "type": "array" })),
        (Some(a_items), None) => a_items,
        (None, Some(b_items)) => b_items.clone(),
        (Some(a_items), Some(b_items)) => {
            unify_item_pair(a_items, b_items, &format!("{}.items", path), config)?
        }
    };
    Some(object([("type", "array".into()), ("items", items)]))
}

/// `check_unifiable_item_schemas` of `[a, b]` with `a` owned, as `unify_pair`. Its scalar
/// promotion and absorption into an array can only succeed for a scalar beside a record
/// (with fields or none) or another scalar, and for an array beside another kind, which go
/// the borrowed way.
fn unify_item_pair(
    a: Value,
    b: &Value,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    let promotes = |x: &Value, y: &Value| {
        is_scalar_schema(x)
            && (is_object_schema(y) || is_empty_record_schema(y) || is_scalar_schema(y))
    };
    let absorbs = is_array_schema(&a) != is_array_schema(b);
    if config.wrap_scalars && (promotes(&a, b) || promotes(b, &a) || absorbs) {
        return check_unifiable_item_schemas(&[&a, b], path, config);
    }
    unify_pair(a, b, path, config)
}

/// `unify_record_schemas` of `[a, b]` with `a` owned, as `unify_pair`: `a`'s fields are
/// moved into the result, those it shares with `b` unified with them in turn.
fn unify_record_pair(
    a: Value,
    b: &Value,
    path: &str,
    config: &SchemaInferenceConfig,
) -> Option<Value> {
    // A field that is an `anyOf` may be unified apart, and placed after the others
    let any_of = |schema: &Value| {
        schema
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| {
                properties.values().any(|field| {
                    normalise_nullable(field)
                        .get("anyOf")
                        .is_some_and(Value::is_array)
                })
            })
    };
    if any_of(&a) || any_of(b) {
        return check_unifiable_schemas(&[&a, b], path, config);
    }
    let has_properties = |schema: &Value| match schema.get("properties") {
        Some(Value::Object(_)) => Some(true),
        _ if is_empty_record_schema(schema) => Some(false),
        _ => None,
    };
    let (a_has, b_has) = (has_properties(&a)?, has_properties(b)?);
    let Value::Object(mut a) = a else {
        unreachable!()
    };
    let a_properties = match a.swap_remove("properties") {
        Some(Value::Object(properties)) if a_has => properties,
        _ => Map::new(),
    };
    let empty = Map::new();
    let b_properties = match b.get("properties") {
        Some(Value::Object(properties)) if b_has => properties,
        _ => &empty,
    };
    // How many of the two require each field
    let a_required = a.swap_remove("required");
    let mut required_in: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for required in [a_required.as_ref(), b.get("required")] {
        if let Some(Value::Array(names)) = required {
            for name in names.iter().filter_map(Value::as_str) {
                *required_in.entry(name).or_default() += 1;
            }
        }
    }
    let required_by_both = |name: &str| required_in.get(name) == Some(&2);

    // The fields of both, in `a`'s order, then those of only `a`, then those of only `b`
    let (in_both, only_a): (Vec<_>, Vec<_>) = a_properties
        .into_iter()
        .map(|(name, field)| {
            let other = b_properties.get(&name).map(normalise_nullable);
            (name, normalise_nullable_owned(field), other)
        })
        .partition(|(_, _, other)| other.is_some());
    let unify = |(name, field, other): (String, Value, Option<&Value>)| {
        let other = other?;
        let unified = if field == *other {
            field
        } else {
            let field_path = format!("{path}.{name}");
            unify_field_pair(Cow::Owned(field), other, &name, &field_path, path, config)?
        };
        Some((name, unified))
    };
    // As many fields as `unify_record_schemas` counts, to go parallel where it does
    let fields = only_a.len() + b_properties.len();
    let in_both: Vec<(String, Value)> = if fields >= 10 {
        in_both.into_par_iter().map(unify).collect::<Option<_>>()?
    } else {
        in_both.into_iter().map(unify).collect::<Option<_>>()?
    };
    let mut required = Vec::new();
    let mut properties = Map::new();
    for (name, field) in in_both {
        let field = if required_by_both(&name) {
            required.push(Value::String(name.clone()));
            field
        } else {
            make_nullable(field)
        };
        properties.insert(name, field);
    }
    for (name, field, _) in only_a {
        properties.insert(name, make_nullable(field));
    }
    for (name, field) in b_properties {
        if !properties.contains_key(name) {
            properties.insert(
                name.clone(),
                make_nullable(normalise_nullable(field).clone()),
            );
        }
    }
    let mut result = object([
        ("type", "object".into()),
        ("properties", Value::Object(properties)),
    ]);
    if !required.is_empty() {
        result["required"] = Value::Array(required);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    include!("../../tests/unification.rs");
}
