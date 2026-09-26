//! Move fields that are invariant per key out of JSON rows into a lookup table.
//!
//! With `spec = [("labels", "id")]`, `labels` is declared invariant for its determinant
//! `id`: every object holding both has `labels` removed, and each distinct `(labels, <id>)`
//! value is kept once in the lookup table, in first-seen order.

use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

/// One distinct subtree removed from the rows.
#[derive(Debug, Clone, PartialEq)]
pub struct LookupEntry {
    /// The removed field's name, e.g. `labels`.
    pub field: String,
    /// The determinant's value, e.g. `Q5`.
    pub key: String,
    /// The removed subtree.
    pub value: Value,
}

/// Slim rows (one per input row, `None` kept as `None`) and their lookup table.
#[derive(Debug)]
pub struct Extracted {
    pub rows: Vec<Option<String>>,
    pub lookup: Vec<LookupEntry>,
}

/// Distinct entries in first-seen order.
#[derive(Default)]
struct Lookup {
    entries: Vec<LookupEntry>,
    index: HashMap<(String, String), usize>,
}

impl Lookup {
    fn insert(&mut self, entry: LookupEntry, determinant: &str) -> Result<(), String> {
        let id = (entry.field.clone(), entry.key.clone());
        match self.index.get(&id) {
            Some(&i) if self.entries[i].value != entry.value => Err(format!(
                "extract_invariants: field '{}' is not invariant for its determinant '{}' \
                 ('{}' has two different values)",
                entry.field, determinant, entry.key
            )),
            Some(_) => Ok(()),
            None => {
                self.index.insert(id, self.entries.len());
                self.entries.push(entry);
                Ok(())
            }
        }
    }
}

/// Remove each `field` whose determinant (its sibling key field) is present, at any depth.
fn strip(node: &mut Value, spec: &[(String, String)], out: &mut Lookup) -> Result<(), String> {
    match node {
        Value::Object(obj) => {
            for (field, key_field) in spec {
                if !obj.contains_key(field) {
                    continue;
                }
                let key = match obj.get(key_field) {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Null) | None => continue,
                    Some(other) => other.to_string(),
                };
                // shift_remove keeps the order of the remaining keys
                let value = obj.shift_remove(field).unwrap();
                out.insert(
                    LookupEntry {
                        field: field.clone(),
                        key,
                        value,
                    },
                    key_field,
                )?;
            }
            obj.values_mut().try_for_each(|v| strip(v, spec, out))
        }
        Value::Array(arr) => arr.iter_mut().try_for_each(|v| strip(v, spec, out)),
        _ => Ok(()),
    }
}

/// Extract the `spec` fields, given as `(field, determinant)` pairs, from every row. Fields
/// found in the same object are recorded in `spec` order. Rows that are not valid JSON
/// pass through unchanged. Errors if a field is not invariant for its determinant, i.e. one
/// determinant value is seen with two different values of the field.
pub fn extract_invariants(
    rows: Vec<Option<String>>,
    spec: &[(String, String)],
) -> Result<Extracted, String> {
    let chunk = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    // Each chunk keeps its own distinct entries, so repeats are dropped as they are seen
    let chunks: Vec<(Vec<Option<String>>, Lookup)> = rows
        .par_chunks(chunk)
        .map(|rows| {
            let mut lookup = Lookup::default();
            let slim = rows
                .iter()
                .map(|row| {
                    let Some(s) = row else { return Ok(None) };
                    let Ok(mut v) = serde_json::from_str::<Value>(s) else {
                        return Ok(Some(s.clone()));
                    };
                    strip(&mut v, spec, &mut lookup)?;
                    Ok(Some(serde_json::to_string(&v).unwrap()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok((slim, lookup))
        })
        .collect::<Result<_, String>>()?;
    drop(rows);

    let mut slim_rows = Vec::new();
    let mut lookup = Lookup::default();
    for (rows, part) in chunks {
        slim_rows.extend(rows);
        for entry in part.entries {
            let determinant = spec
                .iter()
                .find(|(f, _)| *f == entry.field)
                .map_or("", |(_, k)| k.as_str());
            lookup.insert(entry, determinant)?;
        }
    }
    Ok(Extracted {
        rows: slim_rows,
        lookup: lookup.entries,
    })
}

#[cfg(test)]
mod tests {
    include!("tests/extract.rs");
}
