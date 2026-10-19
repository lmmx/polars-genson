//! An Avro schema compiled for normalisation: what normalising a value reads from each
//! schema node (its kind, its fields and their names, which fields `prune` names), read
//! once rather than looked up by key for every value.

use super::{
    apply_map_encoding, field_value, get_scalar_type_from_value, holds, prefix_paths,
    NormaliseConfig, Outcome, PathSegment, PrunedValue,
};
use crate::schema::core::make_promoted_scalar_key;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// A schema node as normalisation reads it.
pub(super) enum Plan<'s> {
    String,
    Int,
    Double,
    Boolean,
    Record(Record<'s>),
    /// An array of its items' plan
    Array(Box<Plan<'s>>),
    /// A map of its values' plan
    Map(Box<Plan<'s>>),
    /// A union holding `null`, with its first other branch: none in `["null"]`, and for
    /// `"null"`, which holds nothing else either
    Nullable(Option<Box<Plan<'s>>>),
    /// A union without `null`, by its first branch: none in `[]`
    Union(Vec<Plan<'s>>),
    /// Any other schema: values are kept as they are
    Passthrough,
}

pub(super) struct Record<'s> {
    /// `None` where the schema's `fields` is not an array: the record is always `{}`
    pub(super) fields: Option<Vec<Field<'s>>>,
    /// Whether a promoted record holds an integer field (see `field_value`)
    pub(super) has_int_field: bool,
    /// The fields named in `prune`: the record is pruned if it holds a value for one
    pub(super) pruning: Vec<&'s str>,
}

/// A record field normalisation writes: one with a name and a type, not named in `prune`.
pub(super) struct Field<'s> {
    pub(super) name: &'s String,
    pub(super) plan: Plan<'s>,
}

impl<'s> Plan<'s> {
    fn new(schema: &'s Value, prune: &HashSet<String>) -> Self {
        fn kind(obj: &Map<String, Value>) -> Option<&str> {
            obj.get("type").and_then(Value::as_str)
        }
        match schema {
            Value::String(t) if t == "string" => Plan::String,
            Value::String(t) if t == "int" || t == "long" => Plan::Int,
            Value::String(t) if t == "double" || t == "float" => Plan::Double,
            Value::String(t) if t == "boolean" => Plan::Boolean,
            Value::String(t) if t == "null" => Plan::Nullable(None),
            Value::Object(obj) if kind(obj) == Some("record") => {
                Plan::Record(Record::new(obj, prune))
            }
            Value::Object(obj) if kind(obj) == Some("array") => {
                Plan::Array(Box::new(Self::of_or_string(obj.get("items"), prune)))
            }
            Value::Object(obj) if kind(obj) == Some("map") => {
                Plan::Map(Box::new(Self::of_or_string(obj.get("values"), prune)))
            }
            Value::Array(types) if types.iter().any(|t| t == "null") => Plan::Nullable(
                types
                    .iter()
                    .find(|t| *t != "null")
                    .map(|branch| Box::new(Plan::new(branch, prune))),
            ),
            Value::Array(types) => Plan::Union(
                types
                    .first()
                    .map(|first| Plan::new(first, prune))
                    .into_iter()
                    .collect(),
            ),
            _ => Plan::Passthrough,
        }
    }

    /// The plan of `schema`, or of `"string"` where there is none.
    fn of_or_string(schema: Option<&'s Value>, prune: &HashSet<String>) -> Self {
        schema.map_or(Plan::String, |schema| Plan::new(schema, prune))
    }
}

impl<'s> Record<'s> {
    fn new(obj: &'s Map<String, Value>, prune: &HashSet<String>) -> Self {
        let Some(Value::Array(fields)) = obj.get("fields") else {
            return Record {
                fields: None,
                has_int_field: false,
                pruning: Vec::new(),
            };
        };
        let names = || fields.iter().filter_map(|f| f.get("name")?.as_str());
        Record {
            // A promoted record may hold both an integer and a float field; an integer
            // goes to the integer one when present, so it lands only once
            has_int_field: names().any(|n| {
                n.contains("__")
                    && matches!(n.rsplit("__").next(), Some("int" | "integer" | "long"))
            }),
            // Names are matched against the schema's fields, so a promoted scalar goes by
            // the name inference gave it
            pruning: names().filter(|n| prune.contains(*n)).collect(),
            fields: Some(
                fields
                    .iter()
                    .filter_map(|f| match (f.get("name"), f.get("type")) {
                        (Some(Value::String(name)), Some(schema)) if !prune.contains(name) => {
                            Some(Field {
                                name,
                                plan: Plan::new(schema, prune),
                            })
                        }
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }
}

/// An Avro schema compiled for normalising values against it, with the same results as
/// `normalise_values_pruned`. Compile a schema once to normalise many values against it.
pub struct NormalisePlan<'s> {
    pub(super) root: Plan<'s>,
    pub(super) cfg: &'s NormaliseConfig,
    /// Nothing can be pruned without `prune`, so a record's input need not stay whole
    take: bool,
}

impl<'s> NormalisePlan<'s> {
    /// `schema` compiled for normalising with `cfg`, pruning the records that hold a field
    /// named in `prune` (see `normalise_values_pruned`).
    pub fn new(schema: &'s Value, cfg: &'s NormaliseConfig, prune: &HashSet<String>) -> Self {
        NormalisePlan {
            root: Plan::new(schema, prune),
            cfg,
            take: prune.is_empty(),
        }
    }

    /// One row normalised, and the values pruned from it (each with `row` 0 and its path
    /// from the row's root): as `normalise_values_pruned` gives them for the row.
    pub fn normalise(&self, value: Value) -> (Value, Vec<PrunedValue>) {
        let mut value = value;
        // Apply wrap_root if requested
        if let Some(ref field) = self.cfg.wrap_root {
            value = Value::Object(std::iter::once((field.clone(), value)).collect());
        }
        let mut pruned = Vec::new();
        // Only the root call passes field name as None
        let out = match self.run(value, &self.root, None, &mut pruned) {
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
        pruned.iter_mut().for_each(|p| p.path.reverse());
        (out, pruned)
    }

    fn run(
        &self,
        value: Value,
        plan: &Plan<'s>,
        field_name: Option<&str>,
        pruned: &mut Vec<PrunedValue>,
    ) -> Outcome {
        use Outcome::Kept;
        let cfg = self.cfg;
        match plan {
            Plan::String => Kept(match value {
                Value::Null => Value::Null,
                v @ Value::String(_) => v,
                v => Value::String(v.to_string()),
            }),

            Plan::Int => Kept(match value {
                Value::Null => Value::Null,
                Value::Number(n) if n.is_i64() => Value::Number(n),
                Value::String(s) if cfg.coerce_string => {
                    s.parse::<i64>().map(|i| json!(i)).unwrap_or(Value::Null)
                }
                _ => Value::Null,
            }),

            Plan::Double => Kept(match value {
                Value::Null => Value::Null,
                // JSON integers in a float field (e.g. 1 alongside 1.5) widen to floats
                Value::Number(n) => n.as_f64().map(|f| json!(f)).unwrap_or(Value::Null),
                Value::String(s) if cfg.coerce_string => {
                    s.parse::<f64>().map(|f| json!(f)).unwrap_or(Value::Null)
                }
                _ => Value::Null,
            }),

            Plan::Boolean => Kept(match value {
                Value::Null => Value::Null,
                Value::Bool(b) => Value::Bool(b),
                Value::String(s) if cfg.coerce_string => match s.as_str() {
                    "true" | "1" => Value::Bool(true),
                    "false" | "0" => Value::Bool(false),
                    _ => Value::Null,
                },
                _ => Value::Null,
            }),

            Plan::Record(record) => {
                let Some(fields) = &record.fields else {
                    return Kept(Value::Object(Map::new()));
                };
                let has_int_field = record.has_int_field;
                if record
                    .pruning
                    .iter()
                    .any(|n| holds(&value, n, has_int_field))
                {
                    return Outcome::Pruned(value);
                }
                let start = pruned.len();
                let mut value = value;
                let mut out = Map::with_capacity(fields.len());
                for Field { name, plan } in fields {
                    let val = field_value(&mut value, name, has_int_field, self.take);
                    let field_start = pruned.len();
                    let normalised = self.run(val, plan, Some(name.as_str()), pruned);
                    prefix_paths(pruned, field_start, || PathSegment::Key((*name).clone()));
                    match normalised {
                        Kept(v) => out.insert((*name).clone(), v),
                        Outcome::Emptied => out.insert((*name).clone(), Value::Null),
                        // A record without one of its fields goes whole, recorded once, as
                        // itself
                        Outcome::Pruned(_) => {
                            pruned.truncate(start);
                            return Outcome::Pruned(value);
                        }
                    };
                }
                Kept(Value::Object(out))
            }

            Plan::Array(items) => match value {
                Value::Null => Kept(Value::Null),
                Value::Array(arr) if arr.is_empty() && cfg.empty_as_null => Kept(Value::Null),
                Value::Array(arr) => {
                    let len = arr.len();
                    let mut out = Vec::with_capacity(len);
                    for (i, v) in arr.into_iter().enumerate() {
                        let start = pruned.len();
                        match self.run(v, items, field_name, pruned) {
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
                v => self.run(Value::Array(vec![v]), plan, field_name, pruned),
            },

            Plan::Map(values) => {
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
                let mut out = Map::new();
                for (k, v) in entries {
                    let start = pruned.len();
                    match self.run(v, values, Some(&k), pruned) {
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

            Plan::Nullable(branch) => match branch {
                // normalise against the first non-null branch
                Some(branch) if !value.is_null() => self.run(value, branch, field_name, pruned),
                // A union of `null` alone (see `normalise_inner`) holds nothing else
                _ => Kept(Value::Null),
            },

            // pick first type
            Plan::Union(first) => self.run(value, &first[0], field_name, pruned),

            Plan::Passthrough => Kept(value),
        }
    }
}
