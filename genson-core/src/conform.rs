//! Typed output conformed to a target schema: a check that an inferred schema fits the
//! target, and the decoded arrays rebuilt in the target's shape, so that chunks inferred
//! independently are written with one schema.
//!
//! A schema `inferred` fits `target` ("compatible") when, recursively:
//! - it is `Null` (no value was seen), which fits any type;
//! - it is the same scalar type (no widening: `Int64` does not fit `Float64`);
//! - both are structs, and each of its fields is one of the target's and fits it. The
//!   target may have fields it lacks: they are written as nulls;
//! - both are lists, both are fixed-size lists of one size, or both are maps, and their
//!   items (or keys and values) fit.
//!
//! Anything else, such as a field the target lacks or another scalar type, is reported
//! rather than dropped or cast. A target field typed `Null` therefore takes only `Null`.

use arrow::array::{
    make_array, new_null_array, Array, ArrayRef, FixedSizeListArray, LargeListArray, ListArray,
    MapArray, StructArray,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, Field, FieldRef, Fields};
use std::sync::Arc;

/// How an inferred schema is checked against a target schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaCheck {
    /// It must fit the target (see the module documentation)
    Compatible,
    /// It must be the target, field for field
    Strict,
    /// Not checked: fields the target lacks are dropped, ones it adds are null, and other
    /// scalar types are cast to the target's
    None,
}

impl std::str::FromStr for SchemaCheck {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "compatible" => Ok(Self::Compatible),
            "strict" => Ok(Self::Strict),
            "none" => Ok(Self::None),
            other => Err(format!(
                "schema_check must be \"compatible\", \"strict\" or \"none\", not {other:?}"
            )),
        }
    }
}

/// The scalar type's Polars name: the types `incompatibilities` compares, and the names
/// its messages use (a string is a string whatever its offsets).
fn scalar_name(dtype: &DataType) -> String {
    match dtype {
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => "String".into(),
        DataType::Binary | DataType::LargeBinary | DataType::BinaryView => "Binary".into(),
        DataType::Boolean => "Boolean".into(),
        DataType::Null => "Null".into(),
        DataType::Int8 => "Int8".into(),
        DataType::Int16 => "Int16".into(),
        DataType::Int32 => "Int32".into(),
        DataType::Int64 => "Int64".into(),
        DataType::UInt8 => "UInt8".into(),
        DataType::UInt16 => "UInt16".into(),
        DataType::UInt32 => "UInt32".into(),
        DataType::UInt64 => "UInt64".into(),
        DataType::Float32 => "Float32".into(),
        DataType::Float64 => "Float64".into(),
        other => format!("{other}"),
    }
}

fn kind(dtype: &DataType) -> String {
    match dtype {
        DataType::Struct(_) => "Struct".into(),
        DataType::List(_) | DataType::LargeList(_) => "List".into(),
        DataType::FixedSizeList(_, n) => format!("Array(size {n})"),
        DataType::Map(..) => "Map".into(),
        other => scalar_name(other),
    }
}

/// A map's key and value fields.
fn map_entries(dtype: &DataType) -> Option<(&FieldRef, &FieldRef)> {
    match dtype {
        DataType::Map(entries, _) => match entries.data_type() {
            DataType::Struct(kv) if kv.len() == 2 => Some((&kv[0], &kv[1])),
            _ => None,
        },
        _ => None,
    }
}

fn item(dtype: &DataType) -> Option<&FieldRef> {
    match dtype {
        DataType::List(f) | DataType::LargeList(f) | DataType::FixedSizeList(f, _) => Some(f),
        _ => None,
    }
}

/// Where `inferred` does not pass `check` against `target`: one line per offending field,
/// its path (`a.b[].c`, `[]` for a list's items, `{}` for a map's values) and why. Empty
/// when it passes, and always under `SchemaCheck::None`.
pub fn incompatibilities(
    inferred: &DataType,
    target: &DataType,
    check: SchemaCheck,
) -> Vec<String> {
    let mut out = Vec::new();
    if check != SchemaCheck::None {
        walk(inferred, target, check, "", &mut out);
    }
    out
}

fn walk(
    inferred: &DataType,
    target: &DataType,
    check: SchemaCheck,
    path: &str,
    out: &mut Vec<String>,
) {
    let at = |p: &str| {
        if p.is_empty() {
            "the root".to_string()
        } else {
            p.to_string()
        }
    };
    if check == SchemaCheck::Compatible && inferred == &DataType::Null {
        return;
    }
    match (inferred, target) {
        (DataType::Struct(have), DataType::Struct(want)) => {
            for f in have {
                let sub = if path.is_empty() {
                    f.name().clone()
                } else {
                    format!("{path}.{}", f.name())
                };
                match want.find(f.name()) {
                    Some((_, w)) => walk(f.data_type(), w.data_type(), check, &sub, out),
                    None => out.push(format!(
                        "{sub}: a field the target schema lacks ({})",
                        kind(f.data_type())
                    )),
                }
            }
            if check == SchemaCheck::Strict {
                for w in want {
                    if have.find(w.name()).is_none() {
                        let sub = if path.is_empty() {
                            w.name().clone()
                        } else {
                            format!("{path}.{}", w.name())
                        };
                        out.push(format!("{sub}: missing, and the schema check is strict"));
                    }
                }
            }
        }
        (DataType::FixedSizeList(_, a), DataType::FixedSizeList(_, b)) if a != b => {
            out.push(format!(
                "{}: Array(size {a}), the target has Array(size {b})",
                at(path)
            ));
        }
        (
            DataType::List(_) | DataType::LargeList(_),
            DataType::List(_) | DataType::LargeList(_),
        )
        | (DataType::FixedSizeList(..), DataType::FixedSizeList(..)) => {
            let (a, b) = (item(inferred).unwrap(), item(target).unwrap());
            walk(
                a.data_type(),
                b.data_type(),
                check,
                &format!("{path}[]"),
                out,
            );
        }
        (DataType::Map(..), DataType::Map(..)) => {
            match (map_entries(inferred), map_entries(target)) {
                (Some((ak, av)), Some((bk, bv))) => {
                    walk(
                        ak.data_type(),
                        bk.data_type(),
                        check,
                        &format!("{path}{{key}}"),
                        out,
                    );
                    walk(
                        av.data_type(),
                        bv.data_type(),
                        check,
                        &format!("{path}{{}}"),
                        out,
                    );
                }
                _ => out.push(format!("{}: a map without a key and a value", at(path))),
            }
        }
        (a, b) if a.is_nested() || b.is_nested() || scalar_name(a) != scalar_name(b) => {
            out.push(format!(
                "{}: {}, the target has {}",
                at(path),
                kind(a),
                kind(b)
            ));
        }
        _ => {}
    }
}

/// `array` in the shape of `target`: struct fields in the target's order, those it lacks
/// as nulls, `Null` arrays as nulls of the target's type, and 32-bit offsets (of lists and
/// strings) widened where the target has 64-bit ones (arrays decoded into
/// [`with_target_offsets`]'s type have them already). Values are not copied; containers
/// are rebuilt over them. Under `SchemaCheck::None` the target's struct fields are taken
/// by name whatever else the array holds, and other scalar types are cast; otherwise the
/// array's schema must already have passed `incompatibilities`.
pub fn conform(array: &ArrayRef, target: &DataType) -> Result<ArrayRef, String> {
    let have = array.data_type();
    if have == target {
        return Ok(array.clone());
    }
    // A `Null` array as nulls of the target's type, and anything under a `Null` target
    // (only reached under `SchemaCheck::None`) as nulls
    if have == &DataType::Null || target == &DataType::Null {
        return Ok(new_null_array(target, array.len()));
    }
    match target {
        DataType::Struct(want) => {
            let s = array
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| format!("{have} does not conform to a struct"))?;
            let columns = want
                .iter()
                .map(|w| match s.column_by_name(w.name()) {
                    Some(c) => conform(c, w.data_type()),
                    None => Ok(new_null_array(w.data_type(), s.len())),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Arc::new(
                StructArray::try_new(nullable(want), columns, s.nulls().cloned())
                    .map_err(|e| e.to_string())?,
            ))
        }
        DataType::List(f) => {
            let list = array
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| format!("{have} does not conform to a list"))?;
            let values = conform(list.values(), f.data_type())?;
            let f = Arc::new(Field::new(f.name(), values.data_type().clone(), true));
            Ok(Arc::new(
                ListArray::try_new(f, list.offsets().clone(), values, list.nulls().cloned())
                    .map_err(|e| e.to_string())?,
            ))
        }
        DataType::LargeList(f) => {
            let (offsets, values, nulls) =
                if let Some(list) = array.as_any().downcast_ref::<ListArray>() {
                    (widen(list.offsets()), list.values(), list.nulls())
                } else {
                    let list = array
                        .as_any()
                        .downcast_ref::<LargeListArray>()
                        .ok_or_else(|| format!("{have} does not conform to a large list"))?;
                    (list.offsets().clone(), list.values(), list.nulls())
                };
            let values = conform(values, f.data_type())?;
            let f = Arc::new(Field::new(f.name(), values.data_type().clone(), true));
            Ok(Arc::new(
                LargeListArray::try_new(f, offsets, values, nulls.cloned())
                    .map_err(|e| e.to_string())?,
            ))
        }
        DataType::FixedSizeList(f, n) => {
            let list = array
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .ok_or_else(|| format!("{have} does not conform to a fixed-size list"))?;
            let values = conform(list.values(), f.data_type())?;
            let f = Arc::new(Field::new(f.name(), values.data_type().clone(), true));
            Ok(Arc::new(
                FixedSizeListArray::try_new(f, *n, values, list.nulls().cloned())
                    .map_err(|e| e.to_string())?,
            ))
        }
        DataType::Map(entries, sorted) => {
            let map = array
                .as_any()
                .downcast_ref::<MapArray>()
                .ok_or_else(|| format!("{have} does not conform to a map"))?;
            let DataType::Struct(want) = entries.data_type() else {
                return Err(format!("{target} has map entries that are not a struct"));
            };
            // The key keeps the target's (non-null) field, so the entries are its type
            let kv = map.entries();
            let columns = want
                .iter()
                .map(|w| match kv.column_by_name(w.name()) {
                    Some(c) => conform(c, w.data_type()),
                    None => Ok(new_null_array(w.data_type(), kv.len())),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let kv = StructArray::try_new(want.clone(), columns, kv.nulls().cloned())
                .map_err(|e| e.to_string())?;
            Ok(Arc::new(
                MapArray::try_new(
                    entries.clone(),
                    map.offsets().clone(),
                    kv,
                    map.nulls().cloned(),
                    *sorted,
                )
                .map_err(|e| e.to_string())?,
            ))
        }
        // Strings to 64-bit offsets (the values buffer is shared), or under
        // `SchemaCheck::None` another scalar type
        _ => arrow::compute::cast(array, target).map_err(|e| e.to_string()),
    }
}

/// `have` with 64-bit offsets wherever `target` has them in its place: a `Utf8` where the
/// target has `LargeUtf8`, a `List` where it has `LargeList`, through struct fields (by
/// name), list items and map keys and values. Everything else is as in `have`, names and
/// nullability included. Arrays decoded into this type conform to `target` as those
/// decoded into `have` do, to the same arrays, with no offsets to widen.
pub fn with_target_offsets(have: &DataType, target: &DataType) -> DataType {
    let field = |f: &FieldRef, t: &DataType| {
        Arc::new(
            f.as_ref()
                .clone()
                .with_data_type(with_target_offsets(f.data_type(), t)),
        )
    };
    match (have, target) {
        (DataType::Utf8, DataType::LargeUtf8) => DataType::LargeUtf8,
        (DataType::List(f), DataType::LargeList(t)) => DataType::LargeList(field(f, t.data_type())),
        (DataType::List(f), DataType::List(t)) => DataType::List(field(f, t.data_type())),
        (DataType::Struct(have), DataType::Struct(want)) => DataType::Struct(
            have.iter()
                .map(|f| match want.find(f.name()) {
                    Some((_, w)) => field(f, w.data_type()),
                    None => f.clone(),
                })
                .collect(),
        ),
        (DataType::Map(entries, sorted), DataType::Map(want, _)) => {
            DataType::Map(field(entries, want.data_type()), *sorted)
        }
        _ => have.clone(),
    }
}

fn widen(offsets: &OffsetBuffer<i32>) -> OffsetBuffer<i64> {
    let wide: Vec<i64> = offsets.iter().map(|&o| i64::from(o)).collect();
    // Widening keeps the offsets ascending from zero or more
    OffsetBuffer::new(wide.into())
}

/// The fields, all nullable, as genson writes them.
fn nullable(fields: &Fields) -> Fields {
    fields
        .iter()
        .map(|f| Arc::new(f.as_ref().clone().with_nullable(true)))
        .collect()
}

/// A struct array's fields as columns, a null row making each of its fields null (as
/// `unnest` in Polars does).
pub fn unnest(array: &StructArray) -> Result<Vec<(FieldRef, ArrayRef)>, String> {
    let rows = array.nulls();
    array
        .fields()
        .iter()
        .zip(array.columns())
        .map(|(f, c)| {
            let column = match rows {
                Some(rows) if rows.null_count() > 0 => {
                    let nulls = NullBuffer::union(Some(rows), c.nulls());
                    make_array(
                        c.to_data()
                            .into_builder()
                            .nulls(nulls)
                            .build()
                            .map_err(|e| e.to_string())?,
                    )
                }
                _ => c.clone(),
            };
            Ok((Arc::new(f.as_ref().clone().with_nullable(true)), column))
        })
        .collect()
}

/// The Arrow type genson writes for a Polars dtype described as JSON by polars-genson's
/// Python package: a dtype name (`"String"`, `"Int64"`, `"Null"`, ...), or
/// `{"Struct": [[name, dtype], ...]}`, `{"List": dtype}`, `{"Array": [dtype, size]}` or
/// `{"Map": [key, value]}`. Strings, binaries and lists have 64-bit offsets, the types
/// Polars writes to Parquet for these dtypes, so that a file written to a target schema
/// is laid out as Polars would write the same frame.
pub fn data_type_from_spec(spec: &serde_json::Value) -> Result<DataType, String> {
    use serde_json::Value;
    let bad = || format!("Unsupported target dtype: {spec}");
    match spec {
        Value::String(name) => Ok(match name.as_str() {
            "String" | "Utf8" => DataType::LargeUtf8,
            "Binary" => DataType::LargeBinary,
            "Boolean" => DataType::Boolean,
            "Null" => DataType::Null,
            "Int8" => DataType::Int8,
            "Int16" => DataType::Int16,
            "Int32" => DataType::Int32,
            "Int64" => DataType::Int64,
            "UInt8" => DataType::UInt8,
            "UInt16" => DataType::UInt16,
            "UInt32" => DataType::UInt32,
            "UInt64" => DataType::UInt64,
            "Float32" => DataType::Float32,
            "Float64" => DataType::Float64,
            _ => return Err(bad()),
        }),
        Value::Object(o) if o.len() == 1 => {
            let (kind, inner) = o.iter().next().expect("one entry");
            match (kind.as_str(), inner) {
                ("Struct", Value::Array(fields)) => {
                    let fields = fields
                        .iter()
                        .map(|f| match f.as_array().map(Vec::as_slice) {
                            Some([Value::String(name), dtype]) => {
                                Ok(Field::new(name, data_type_from_spec(dtype)?, true))
                            }
                            _ => Err(bad()),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(DataType::Struct(fields.into()))
                }
                ("List", inner) => Ok(DataType::new_large_list(data_type_from_spec(inner)?, true)),
                ("Array", Value::Array(parts)) => match parts.as_slice() {
                    [inner, Value::Number(n)] => {
                        let size = n
                            .as_i64()
                            .and_then(|n| i32::try_from(n).ok())
                            .ok_or_else(bad)?;
                        Ok(DataType::new_fixed_size_list(
                            data_type_from_spec(inner)?,
                            size,
                            true,
                        ))
                    }
                    _ => Err(bad()),
                },
                ("Map", Value::Array(parts)) => match parts.as_slice() {
                    [key, value] => {
                        let entries = DataType::Struct(
                            vec![
                                Field::new("key", data_type_from_spec(key)?, false),
                                Field::new("value", data_type_from_spec(value)?, true),
                            ]
                            .into(),
                        );
                        Ok(DataType::Map(
                            Arc::new(Field::new("entries", entries, false)),
                            false,
                        ))
                    }
                    _ => Err(bad()),
                },
                _ => Err(bad()),
            }
        }
        _ => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    include!("tests/conform.rs");
}
