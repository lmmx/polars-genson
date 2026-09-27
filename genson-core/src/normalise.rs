use crate::schema::core::make_promoted_scalar_key;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapEncoding {
    /// Avro/JSON-style object: {"en":"Hello","fr":"Bonjour"}
    Mapping,
    /// List of single-entry objects: [{"en":"Hello"},{"fr":"Bonjour"}]
    Entries,
    #[serde(rename = "kv")]
    /// List of {key,value} pairs: [{"key":"en","value":"Hello"}, {"key":"fr","value":"Bonjour"}]
    KeyValueEntries,
}

/// Configuration options for normalisation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NormaliseConfig {
    /// Whether empty arrays/maps should be normalised to `null` (default: true).
    pub empty_as_null: bool,
    /// Whether to try to coerce int/float/bool from string (default: false).
    pub coerce_string: bool,
    /// Which map encoding to output Map type fields into (default: Mapping).
    pub map_encoding: MapEncoding,
    /// Optional: wrap input values inside an object with this field name
    pub wrap_root: Option<String>,
}

impl Default for NormaliseConfig {
    fn default() -> Self {
        Self {
            empty_as_null: true,
            coerce_string: false,
            map_encoding: MapEncoding::Mapping,
            wrap_root: None,
        }
    }
}

/// Apply map encoding strategy to a map of already-normalised values.
fn apply_map_encoding(m: serde_json::Map<String, Value>, encoding: MapEncoding) -> Value {
    match encoding {
        MapEncoding::Mapping => Value::Object(m),
        MapEncoding::Entries => {
            let arr: Vec<Value> = m.into_iter().map(|(k, v)| json!({ k: v })).collect();
            Value::Array(arr)
        }
        MapEncoding::KeyValueEntries => {
            let arr: Vec<Value> = m
                .into_iter()
                .map(|(k, v)| json!({ "key": k, "value": v }))
                .collect();
            Value::Array(arr)
        }
    }
}

/// A step on the path from a row's root to a value: a record field or map key, or an
/// array index.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

/// A value removed by `prune` (see `normalise_values_pruned`), as it was in the input.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PrunedValue {
    pub row: usize,
    pub path: Vec<PathSegment>,
    pub value: Value,
}

/// A value normalised, pruned (carrying its input value up to where it is removed), or
/// emptied by pruning (its pruned contents are already recorded).
enum Outcome {
    Kept(Value),
    Pruned(Value),
    Emptied,
}

/// Add `seg` to the (reversed) paths of the values pruned since `start`.
fn prefix_paths(pruned: &mut [PrunedValue], start: usize, seg: impl FnOnce() -> PathSegment) {
    if let Some(new) = pruned.get_mut(start..).filter(|new| !new.is_empty()) {
        let seg = seg();
        new.iter_mut().for_each(|p| p.path.push(seg.clone()));
    }
}

/// Whether a scalar in place of a record goes to the record's promoted field `name`
/// (e.g. a string to `mainsnak__string`).
fn promoted_scalar_matches(scalar: &Value, name: &str, has_int_field: bool) -> bool {
    if !name.contains("__") {
        return false;
    }
    match (scalar, name.split("__").last().unwrap_or("")) {
        (Value::String(_), "string") => true,
        (Value::Bool(_), "boolean") => true,
        (Value::Number(n), "int" | "integer" | "long") => n.is_i64() || n.is_u64(),
        (Value::Number(n), "float" | "double" | "number") => n.is_f64() || !has_int_field,
        _ => false,
    }
}

/// The input value of a record's field `name`.
fn field_value(value: &Value, name: &str, has_int_field: bool) -> Value {
    match value {
        Value::Object(m) => m.get(name).cloned().unwrap_or(Value::Null),
        scalar if promoted_scalar_matches(scalar, name, has_int_field) => scalar.clone(),
        _ => Value::Null,
    }
}

/// Whether a record's input value holds a non-null value for its field `name`.
fn holds(value: &Value, name: &str, has_int_field: bool) -> bool {
    match value {
        Value::Object(m) => m.get(name).is_some_and(|v| !v.is_null()),
        Value::Null => false,
        scalar => promoted_scalar_matches(scalar, name, has_int_field),
    }
}

/// Remove the record fields named in `prune` from an Avro schema, at any depth.
pub fn prune_schema(schema: &mut Value, prune: &HashSet<String>) {
    match schema {
        Value::Object(obj) => {
            if obj.get("type") == Some(&Value::String("record".into())) {
                if let Some(Value::Array(fields)) = obj.get_mut("fields") {
                    fields.retain(|f| {
                        !f.get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|n| prune.contains(n))
                    });
                }
            }
            obj.values_mut().for_each(|v| prune_schema(v, prune));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| prune_schema(v, prune)),
        _ => {}
    }
}

fn get_scalar_type_from_value(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(n) if n.is_i64() => "int",
        Value::Number(_) => "float",
        Value::Bool(_) => "boolean",
        _ => "unknown",
    }
}

/// Normalise a single JSON value against an Avro schema.
///
/// This function takes *jagged* or irregular JSON data and reshapes it into a
/// **consistent, schema-aligned form**. It ensures every value conforms to the
/// expectations of the provided Avro schema, filling gaps, coercing types, and
/// handling nullability in a predictable way.
///
/// It is primarily intended for normalising semi-structured JSON columns (e.g.
/// in a dataframe) so that downstream processing sees stable, predictable shapes
/// instead of row-by-row variation.
///
/// By default, string values are *not* coerced into numbers/booleans. Use
/// `coerce_string = true` to enable parsing `"42"` → `42`, `"true"` → `true`, etc.
///
/// ## Behaviour by schema type
///
/// - **Primitive types** (`"string"`, `"int"`, `"long"`, `"double"`, `"float"`,
///   `"boolean"`):
///   * `null` is always preserved as `null`.
///   * String values are parsed into the target type where possible
///     (`"42"` → `42`, `"true"` → `true`) if `coerce_string` is true.
///   * If parsing fails, the value becomes `null`.
///   * Non-matching values are coerced to string via `.to_string()` for the
///     `"string"` type, or dropped to `null` for numeric/boolean types.
///
/// - **Record** (`{"type":"record","fields":[...]}`):
///   * Produces a JSON object with exactly the schema’s fields.
///   * Missing fields are filled with `null`.
///   * Extra fields in the input are ignored.
///   * Each field is recursively normalised against its declared type.
///
/// - **Array** (`{"type":"array","items": ...}`):
///   * `null` stays `null`.
///   * Empty arrays become `null` if `cfg.empty_as_null == true`,
///     otherwise they remain empty arrays, which can help to avoid row elimination
///     when flattened/'exploded'.
///   * Non-array values are wrapped in a singleton array and normalised
///     against the `items` schema.
///   * Elements are recursively normalised.
///
/// - **Map** (`{"type":"map","values": ...}`):
///   * `null` stays `null`.
///   * Empty objects become `null` if `cfg.empty_as_null == true`,
///     otherwise they remain empty objects, which can help to avoid row elimination
///     when flattened/unnested.
///   * Each entry’s value is recursively normalised against the `values` schema.
///   * Non-object values are coerced into a single-entry object
///     (`{"default": value}`).
///
/// - **Union** (`[ ... ]`):
///   * If the union contains `"null"`, then `null` inputs are preserved.
///   * Otherwise, values are normalised against the **first non-null branch**.
///   * For multi-type unions without `"null"`, only the **first branch**
///     is considered. Union order therefore determines precedence
///     (e.g. `["string","int"]` coerces numbers to strings, while
///     `["int","string"]` parses strings as integers).
///
/// - **Fallback**:
///   * If the schema is not recognised, the input value is returned unchanged.
///
/// ## Config options
///
/// - `empty_as_null`: when true, empty arrays and empty objects (maps)
///   are replaced with `null` instead of being preserved.
///
/// ## Notes
///
/// * This implementation prioritises schema consistency over fidelity.
///   Data may be dropped (`null`) or coerced (e.g. numbers to strings) if
///   it does not match the schema.
/// * Avro’s full union semantics are simplified here: only the first matching
///   branch is tried, not all possible branches.
pub fn normalise_value(
    value: Value,
    schema: &Value,
    cfg: &NormaliseConfig,
    field_name: Option<&str>,
) -> Value {
    match normalise_inner(value, schema, cfg, field_name, &HashSet::new(), &mut Vec::new()) {
        Outcome::Kept(v) => v,
        // Only reachable with a non-empty `prune`
        Outcome::Pruned(_) | Outcome::Emptied => Value::Null,
    }
}

/// `normalise_value`, pruning the records that hold a field named in `prune` into
/// `pruned` (see `normalise_values_pruned`).
fn normalise_inner(
    value: Value,
    schema: &Value,
    cfg: &NormaliseConfig,
    field_name: Option<&str>,
    prune: &HashSet<String>,
    pruned: &mut Vec<PrunedValue>,
) -> Outcome {
    use Outcome::Kept;
    match schema {
        // Primitive types
        Value::String(t) if t == "string" => Kept(match value {
            Value::Null => Value::Null,
            v @ Value::String(_) => v,
            v => Value::String(v.to_string()),
        }),

        Value::String(t) if t == "int" || t == "long" => Kept(match value {
            Value::Null => Value::Null,
            Value::Number(n) if n.is_i64() => Value::Number(n),
            Value::String(s) if cfg.coerce_string => {
                s.parse::<i64>().map(|i| json!(i)).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        }),

        Value::String(t) if t == "double" || t == "float" => Kept(match value {
            Value::Null => Value::Null,
            // JSON integers in a float field (e.g. 1 alongside 1.5) widen to floats
            Value::Number(n) => n.as_f64().map(|f| json!(f)).unwrap_or(Value::Null),
            Value::String(s) if cfg.coerce_string => {
                s.parse::<f64>().map(|f| json!(f)).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        }),

        Value::String(t) if t == "boolean" => Kept(match value {
            Value::Null => Value::Null,
            Value::Bool(b) => Value::Bool(b),
            Value::String(s) if cfg.coerce_string => match s.as_str() {
                "true" | "1" => Value::Bool(true),
                "false" | "0" => Value::Bool(false),
                _ => Value::Null,
            },
            _ => Value::Null,
        }),

        // Record
        Value::Object(obj) if obj.get("type") == Some(&Value::String("record".into())) => {
            let mut out = serde_json::Map::new();
            if let Some(Value::Array(fields)) = obj.get("fields") {
                // A promoted record may hold both an integer and a float field; an
                // integer goes to the integer one when present, so it lands only once
                let has_int_field = fields.iter().any(|f| {
                    f.get("name").and_then(|n| n.as_str()).is_some_and(|n| {
                        n.contains("__")
                            && matches!(n.rsplit("__").next(), Some("int" | "integer" | "long"))
                    })
                });
                let is_pruned = |f: &Value| {
                    f.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|n| prune.contains(n))
                };
                if !prune.is_empty()
                    && fields.iter().any(|f| {
                        is_pruned(f)
                            && holds(&value, f["name"].as_str().unwrap(), has_int_field)
                    })
                {
                    return Outcome::Pruned(value);
                }
                let start = pruned.len();
                for f in fields {
                    if let (Some(Value::String(name)), Some(field_schema)) =
                        (f.get("name"), f.get("type"))
                    {
                        if !prune.is_empty() && prune.contains(name) {
                            continue;
                        }
                        let val = field_value(&value, name, has_int_field);
                        let field_start = pruned.len();
                        let normalised =
                            normalise_inner(val, field_schema, cfg, Some(name), prune, pruned);
                        prefix_paths(pruned, field_start, || PathSegment::Key(name.clone()));
                        match normalised {
                            Kept(v) => out.insert(name.clone(), v),
                            Outcome::Emptied => out.insert(name.clone(), Value::Null),
                            // A record without one of its fields goes whole, recorded
                            // once, as itself
                            Outcome::Pruned(_) => {
                                pruned.truncate(start);
                                return Outcome::Pruned(value);
                            }
                        };
                    }
                }
            }
            Kept(Value::Object(out))
        }

        // Array
        Value::Object(obj) if obj.get("type") == Some(&Value::String("array".into())) => {
            let default_items = Value::String("string".into());
            let items_schema = obj.get("items").unwrap_or(&default_items);
            match value {
                Value::Null => Kept(Value::Null),
                Value::Array(arr) if arr.is_empty() && cfg.empty_as_null => Kept(Value::Null),
                Value::Array(arr) => {
                    let len = arr.len();
                    let mut out = Vec::with_capacity(len);
                    for (i, v) in arr.into_iter().enumerate() {
                        let start = pruned.len();
                        match normalise_inner(v, items_schema, cfg, field_name, prune, pruned) {
                            Kept(v) => out.push(v),
                            Outcome::Pruned(v) => pruned.push(PrunedValue {
                                row: 0,
                                path: Vec::new(),
                                value: v,
                            }),
                            Outcome::Emptied => {}
                        }
                        prefix_paths(pruned, start, || PathSegment::Index(i));
                    }
                    if out.is_empty() && len > 0 {
                        Outcome::Emptied
                    } else {
                        Kept(Value::Array(out))
                    }
                }
                v => normalise_inner(Value::Array(vec![v]), schema, cfg, field_name, prune, pruned),
            }
        }

        // Map
        Value::Object(obj) if obj.get("type") == Some(&Value::String("map".into())) => {
            let default_values = Value::String("string".into());
            let values_schema = obj.get("values").unwrap_or(&default_values);

            let entries = match value {
                Value::Null => return Kept(Value::Null),
                Value::Object(m) if m.is_empty() && cfg.empty_as_null => {
                    return Kept(Value::Null)
                }
                Value::Object(m) => m,
                // Scalar fallback: wrap under a promoted key
                v => {
                    let scalar_type = get_scalar_type_from_value(&v);
                    let wrapped_key =
                        make_promoted_scalar_key(field_name.unwrap_or(""), scalar_type);
                    std::iter::once((wrapped_key, v)).collect()
                }
            };
            let len = entries.len();
            let mut out = serde_json::Map::new();
            for (k, v) in entries {
                let start = pruned.len();
                match normalise_inner(v, values_schema, cfg, Some(&k), prune, pruned) {
                    Kept(v) => {
                        prefix_paths(pruned, start, || PathSegment::Key(k.clone()));
                        out.insert(k, v);
                    }
                    Outcome::Pruned(v) => {
                        pruned.push(PrunedValue {
                            row: 0,
                            path: Vec::new(),
                            value: v,
                        });
                        prefix_paths(pruned, start, || PathSegment::Key(k));
                    }
                    Outcome::Emptied => prefix_paths(pruned, start, || PathSegment::Key(k)),
                }
            }
            if out.is_empty() && len > 0 {
                Outcome::Emptied
            } else {
                Kept(apply_map_encoding(out, cfg.map_encoding))
            }
        }

        // Union
        Value::Array(types) => {
            // Typical Avro union is ["null", T]
            if types.iter().any(|t| t == "null") {
                if value.is_null() {
                    Kept(Value::Null)
                } else {
                    // normalise against the first non-null branch
                    let branch = types.iter().find(|t| *t != "null").unwrap();
                    normalise_inner(value, branch, cfg, field_name, prune, pruned)
                }
            } else {
                // pick first type
                normalise_inner(value, &types[0], cfg, field_name, prune, pruned)
            }
        }

        // Fallback: just return value
        _ => Kept(value),
    }
}

/// Normalise a list of JSON values (e.g. a column in Polars).
pub fn normalise_values(values: Vec<Value>, schema: &Value, cfg: &NormaliseConfig) -> Vec<Value> {
    normalise_values_pruned(values, schema, cfg, &HashSet::new()).0
}

/// Normalise a list of JSON values, pruning the records that hold a non-null value for a
/// field named in `prune`. Names are record fields of the schema, at any depth; a scalar
/// promoted to a record goes by its promoted field (e.g. `mainsnak__string`).
///
/// - A pruned record that is a field of another record takes that record with it, up to
///   an array element or map entry, which is removed on its own. Reaching the root makes
///   the row null.
/// - An array or map left empty by pruning is removed from its array or map, or is null
///   as a record field. One empty in the input is normalised as usual.
/// - The named fields are left out of every record (see `prune_schema`).
///
/// Returns the rows and the removed values, each with its row, its path from the row's
/// root (after `wrap_root`), and its input value.
pub fn normalise_values_pruned(
    values: Vec<Value>,
    schema: &Value,
    cfg: &NormaliseConfig,
    prune: &HashSet<String>,
) -> (Vec<Value>, Vec<PrunedValue>) {
    let mut pruned = Vec::new();
    let rows = values
        .into_iter()
        .enumerate()
        .map(|(row, mut v)| {
            // Apply wrap_root if requested
            if let Some(ref field) = cfg.wrap_root {
                v = Value::Object(
                    std::iter::once((field.clone(), v)).collect::<serde_json::Map<String, Value>>(),
                );
            }
            let start = pruned.len();
            // Only the root call passes field name as None
            let out = match normalise_inner(v, schema, cfg, None, prune, &mut pruned) {
                Outcome::Kept(v) => v,
                Outcome::Pruned(v) => {
                    pruned.push(PrunedValue {
                        row: 0,
                        path: Vec::new(),
                        value: v,
                    });
                    Value::Null
                }
                Outcome::Emptied => Value::Null,
            };
            for p in &mut pruned[start..] {
                p.row = row;
                p.path.reverse();
            }
            out
        })
        .collect();
    (rows, pruned)
}

#[cfg(test)]
mod tests {
    include!("tests/normalise.rs");
}
