#[cfg_attr(feature = "trace", crustrace::omni)]
mod innermod {
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
    }

    impl Default for NormaliseConfig {
        fn default() -> Self {
            Self {
                empty_as_null: true,
                coerce_string: false,
                map_encoding: MapEncoding::Mapping,
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
    pub fn normalise_value(value: Value, schema: &Value, cfg: &NormaliseConfig) -> Value {
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
                                _ => Value::Null,
                            };
                            out.insert(name.clone(), normalise_value(val, field_schema, cfg));
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
                            .map(|v| normalise_value(v, items_schema, cfg))
                            .collect(),
                    ),
                    v => Value::Array(vec![normalise_value(v, items_schema, cfg)]),
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
                        for (k, v) in m {
                            out.insert(k, normalise_value(v, values_schema, cfg));
                        }
                        apply_map_encoding(out, cfg.map_encoding)
                    }
                    v => {
                        // Scalar fallback: wrap as {"default": v}
                        let mut synthetic = serde_json::Map::new();
                        synthetic.insert("default".into(), normalise_value(v, values_schema, cfg));
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
                        normalise_value(value, branch, cfg)
                    }
                } else {
                    // pick first type
                    normalise_value(value, &types[0], cfg)
                }
            }

            // Fallback: just return value
            _ => value,
        }
    }

    /// Normalise a list of JSON values (e.g. a column in Polars).
    pub fn normalise_values(
        values: Vec<Value>,
        schema: &Value,
        cfg: &NormaliseConfig,
    ) -> Vec<Value> {
        values
            .into_iter()
            .map(|v| normalise_value(v, schema, cfg))
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn test_normalise_record() {
            let schema = json!({
                "type": "record",
                "name": "doc",
                "fields": [
                    {"name": "id", "type": "string"},
                    {"name": "labels", "type": {"type": "map", "values": "string"}},
                ]
            });

            let cfg = NormaliseConfig::default();
            let input = json!({"id": 42}); // id is number, labels missing
            let normalised = normalise_value(input, &schema, &cfg);

            assert_eq!(normalised, json!({"id": "42", "labels": Value::Null}));
        }

        #[test]
        fn test_normalise_array_union() {
            let schema = json!(["null", {"type": "array", "items": "string"}]);
            let cfg = NormaliseConfig::default();

            let input = json!("hello"); // scalar string
            let normalised = normalise_value(input, &schema, &cfg);

            assert_eq!(normalised, json!(["hello"]));
        }

        #[test]
        fn test_empty_map_to_null() {
            let schema = json!({"type": "map", "values": "string"});
            let cfg = NormaliseConfig::default();

            let input = json!({});
            let normalised = normalise_value(input, &schema, &cfg);

            assert_eq!(normalised, Value::Null);
        }

        #[test]
        fn test_empty_map_preserved_if_flag_off() {
            let schema = json!({"type": "map", "values": "string"});
            let cfg = NormaliseConfig {
                empty_as_null: false,
                ..NormaliseConfig::default()
            };

            let input = json!({});
            let normalised = normalise_value(input, &schema, &cfg);

            assert_eq!(normalised, json!({}));
        }

        #[test]
        fn test_string_coercion_toggle() {
            let schema = json!({
                "type": "record",
                "name": "doc",
                "fields": [
                    {"name": "int_field", "type": "int"},
                    {"name": "bool_field", "type": "boolean"},
                ]
            });

            let input = json!({
                "int_field": "42",
                "bool_field": "true"
            });

            // Default: coerce_string = false
            let cfg_no_coerce = NormaliseConfig {
                empty_as_null: true,
                coerce_string: false,
                ..NormaliseConfig::default()
            };
            let norm_no_coerce = normalise_value(input.clone(), &schema, &cfg_no_coerce);
            assert_eq!(
                norm_no_coerce,
                json!({
                    "int_field": Value::Null,     // stays null because string not coerced
                    "bool_field": Value::Null     // same here
                })
            );

            // With coerce_string = true
            let cfg_coerce = NormaliseConfig {
                empty_as_null: true,
                coerce_string: true,
                ..NormaliseConfig::default()
            };
            let norm_coerce = normalise_value(input, &schema, &cfg_coerce);
            assert_eq!(
                norm_coerce,
                json!({
                    "int_field": 42,
                    "bool_field": true
                })
            );
        }
    }
}
pub use innermod::*;
