use crate::schema::core::make_promoted_scalar_key;
use serde_json::{json, Value};

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
#[derive(Debug, Clone)]
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
    match schema {
        // Primitive types
        Value::String(t) if t == "string" => match value {
            Value::Null => Value::Null,
            v @ Value::String(_) => v,
            v => Value::String(v.to_string()),
        },

        Value::String(t) if t == "int" || t == "long" => match value {
            Value::Null => Value::Null,
            Value::Number(n) if n.is_i64() => Value::Number(n),
            Value::String(s) if cfg.coerce_string => {
                s.parse::<i64>().map(|i| json!(i)).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        },

        Value::String(t) if t == "double" || t == "float" => match value {
            Value::Null => Value::Null,
            Value::Number(n) if n.is_f64() => Value::Number(n),
            Value::String(s) if cfg.coerce_string => {
                s.parse::<f64>().map(|f| json!(f)).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        },

        Value::String(t) if t == "boolean" => match value {
            Value::Null => Value::Null,
            Value::Bool(b) => Value::Bool(b),
            Value::String(s) if cfg.coerce_string => match s.as_str() {
                "true" | "1" => Value::Bool(true),
                "false" | "0" => Value::Bool(false),
                _ => Value::Null,
            },
            _ => Value::Null,
        },

        // Record
        Value::Object(obj) if obj.get("type") == Some(&Value::String("record".into())) => {
            let mut out = serde_json::Map::new();
            if let Some(Value::Array(fields)) = obj.get("fields") {
                for f in fields {
                    if let (Some(Value::String(name)), Some(field_schema)) =
                        (f.get("name"), f.get("type"))
                    {
                        let val = match &value {
                            Value::Object(m) => m.get(name).cloned().unwrap_or(Value::Null),
                            // Handle scalar promotion case
                            scalar_value => {
                                // If this is a synthetic field that matches the scalar type
                                if name.contains("__") {
                                    let type_suffix = name.split("__").last().unwrap_or("");
                                    let matches_type = matches!(
                                        (scalar_value, type_suffix),
                                        (Value::String(_), "string")
                                            | (
                                                Value::Number(_),
                                                "int"
                                                    | "integer"
                                                    | "long"
                                                    | "float"
                                                    | "double"
                                                    | "number",
                                            )
                                            | (Value::Bool(_), "boolean")
                                    );

                                    if matches_type {
                                        scalar_value.clone()
                                    } else {
                                        Value::Null
                                    }
                                } else {
                                    Value::Null
                                }
                            }
                        };
                        out.insert(
                            name.clone(),
                            normalise_value(val, field_schema, cfg, Some(name)),
                        );
                    }
                }
            }
            Value::Object(out)
        }

        // Array
        Value::Object(obj) if obj.get("type") == Some(&Value::String("array".into())) => {
            let default_items = Value::String("string".into());
            let items_schema = obj.get("items").unwrap_or(&default_items);
            match value {
                Value::Null => Value::Null,
                Value::Array(arr) if arr.is_empty() && cfg.empty_as_null => Value::Null,
                Value::Array(arr) => Value::Array(
                    arr.into_iter()
                        .map(|v| normalise_value(v, items_schema, cfg, field_name))
                        .collect(),
                ),
                v => Value::Array(vec![normalise_value(v, items_schema, cfg, field_name)]),
            }
        }

        // Map
        Value::Object(obj) if obj.get("type") == Some(&Value::String("map".into())) => {
            let default_values = Value::String("string".into());
            let values_schema = obj.get("values").unwrap_or(&default_values);

            match value {
                Value::Null => Value::Null,

                Value::Object(m) if m.is_empty() && cfg.empty_as_null => Value::Null,

                Value::Object(m) => {
                    let mut out = serde_json::Map::new();

                    if values_schema.get("type") == Some(&Value::String("object".into())) {
                        // --- Map of records ---
                        for (k, v) in m {
                            let normalised_record =
                                normalise_value(v, values_schema, cfg, Some(&k));
                            out.insert(k, normalised_record);
                        }
                    } else {
                        // --- Map of scalars (existing behaviour) ---
                        for (k, v) in m {
                            let normalised_value = normalise_value(v, values_schema, cfg, Some(&k));
                            out.insert(k, normalised_value);
                        }
                    }

                    apply_map_encoding(out, cfg.map_encoding)
                }

                v => {
                    // Scalar fallback: wrap as {"default": v}
                    let mut synthetic = serde_json::Map::new();
                    let scalar_type = get_scalar_type_from_value(&v);
                    let wrapped_key =
                        make_promoted_scalar_key(field_name.unwrap_or(""), scalar_type);
                    synthetic.insert(
                        wrapped_key,
                        normalise_value(v, values_schema, cfg, field_name),
                    );
                    apply_map_encoding(synthetic, cfg.map_encoding)
                }
            }
        }

        // Union
        Value::Array(types) => {
            // Typical Avro union is ["null", T]
            if types.iter().any(|t| t == "null") {
                if value.is_null() {
                    Value::Null
                } else {
                    // normalise against the first non-null branch
                    let branch = types.iter().find(|t| *t != "null").unwrap();
                    normalise_value(value, branch, cfg, field_name)
                }
            } else {
                // pick first type
                normalise_value(value, &types[0], cfg, field_name)
            }
        }

        // Fallback: just return value
        _ => value,
    }
}

/// Normalise a list of JSON values (e.g. a column in Polars).
pub fn normalise_values(values: Vec<Value>, schema: &Value, cfg: &NormaliseConfig) -> Vec<Value> {
    values
        .into_iter()
        .map(|mut v| {
            // Apply wrap_root if requested
            if let Some(ref field) = cfg.wrap_root {
                v = Value::Object(
                    std::iter::once((field.clone(), v)).collect::<serde_json::Map<String, Value>>(),
                );
            }
            normalise_value(v, schema, cfg, None) // Only the root call passes field name as None
        })
        .collect()
}

#[cfg(test)]
mod tests {
    include!("tests/normalise.rs");
}
