//! Move fields that are invariant per key out of JSON rows into a lookup table.
//!
//! With `spec = [("labels", "id")]`, `labels` is declared invariant for its determinant
//! `id`: every object holding both has `labels` removed, and each distinct `(labels, <id>)`
//! value is kept once in the lookup table, in first-seen order.

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde_json::Value;

pub(crate) mod scan;

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

/// A chunk of rows extracted: its slim rows and its lookup entries.
type Chunk = (Vec<Option<String>>, Lookup);

/// A removed field's value as a lookup holds it: its JSON text where the row was scanned,
/// and its `Value`, parsed from the text only once it is needed.
struct Held {
    text: Option<String>,
    value: Option<Value>,
    /// How deeply the text nests, where it was scanned.
    nesting: usize,
}

impl Held {
    fn value(&mut self) -> &Value {
        let Held { text, value, .. } = self;
        value.get_or_insert_with(|| {
            let text = text
                .as_deref()
                .expect("a held value has its text or its value");
            serde_json::from_str(text).expect("a field's text was validated by the scan")
        })
    }

    /// Whether two held values are equal: by their texts when both have one and they are
    /// the same, else by their values.
    fn same(&mut self, other: &mut Held) -> bool {
        match (&self.text, &other.text) {
            (Some(a), Some(b)) if a == b => true,
            _ => self.value() == other.value(),
        }
    }
}

/// Distinct entries in first-seen order.
#[derive(Default)]
struct Lookup {
    ids: Vec<(String, String)>,
    held: Vec<Held>,
    /// Each entry's index, by field and then key.
    index: FxHashMap<String, FxHashMap<String, usize>>,
}

fn not_invariant(field: &str, determinant: &str, key: &str) -> String {
    format!(
        "extract_invariants: field '{}' is not invariant for its determinant '{}' \
         ('{}' has two different values)",
        field, determinant, key
    )
}

impl Lookup {
    fn position(&self, field: &str, key: &str) -> Option<usize> {
        self.index.get(field)?.get(key).copied()
    }

    fn insert_held(
        &mut self,
        field: &str,
        key: &str,
        mut held: Held,
        determinant: &str,
    ) -> Result<(), String> {
        match self.position(field, key) {
            Some(i) if !self.held[i].same(&mut held) => Err(not_invariant(field, determinant, key)),
            Some(_) => Ok(()),
            None => {
                let at = self.held.len();
                self.index
                    .entry(field.to_string())
                    .or_default()
                    .insert(key.to_string(), at);
                self.ids.push((field.to_string(), key.to_string()));
                self.held.push(held);
                Ok(())
            }
        }
    }

    fn insert(&mut self, entry: LookupEntry, determinant: &str) -> Result<(), String> {
        let held = Held {
            text: None,
            value: Some(entry.value),
            nesting: 0,
        };
        self.insert_held(&entry.field, &entry.key, held, determinant)
    }

    /// `insert` of a field's value given as its JSON text, held as text: nothing is copied
    /// when the entry already holds this text.
    fn insert_text(
        &mut self,
        field: &str,
        key: &str,
        text: &str,
        nesting: usize,
        determinant: &str,
    ) -> Result<(), String> {
        if let Some(i) = self.position(field, key) {
            if self.held[i].text.as_deref() == Some(text) {
                return Ok(());
            }
        }
        let held = Held {
            text: Some(text.to_string()),
            value: None,
            nesting,
        };
        self.insert_held(field, key, held, determinant)
    }

    fn into_entries(self) -> Vec<LookupEntry> {
        self.ids
            .into_iter()
            .zip(self.held)
            .map(|((field, key), mut held)| {
                held.value();
                LookupEntry {
                    field,
                    key,
                    value: held.value.unwrap(),
                }
            })
            .collect()
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
    extract_invariants_from(&rows, spec)
}

/// [`extract_invariants`] on borrowed rows, such as those of a
/// [`crate::parquet::StringColumn`].
pub fn extract_invariants_from<S: AsRef<str> + Sync>(
    rows: &[Option<S>],
    spec: &[(String, String)],
) -> Result<Extracted, String> {
    extract(rows, spec, true)
}

/// [`extract_invariants_from`] with every row read as a `Value`, never by scanning its
/// text: the reference the scan is tested against. The slim rows are written as
/// `serde_json` writes them, where the scan keeps the row's own text.
#[doc(hidden)]
pub fn extract_invariants_by_value<S: AsRef<str> + Sync>(
    rows: &[Option<S>],
    spec: &[(String, String)],
) -> Result<Extracted, String> {
    extract(rows, spec, false)
}

fn extract<S: AsRef<str> + Sync>(
    rows: &[Option<S>],
    spec: &[(String, String)],
    scan_rows: bool,
) -> Result<Extracted, String> {
    let chunk = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    // Each chunk keeps its own distinct entries, so repeats are dropped as they are seen
    // Each chunk's result is kept, so that the error reported is the first in row order
    // rather than whichever chunk failed first
    let chunks: Vec<Result<Chunk, String>> = rows
        .par_chunks(chunk)
        .map(|rows| {
            let mut lookup = Lookup::default();
            let slim = rows
                .iter()
                .map(|row| {
                    let Some(s) = row else { return Ok(None) };
                    let s = s.as_ref();
                    // One pass over the row's text when it can be read exactly so, else
                    // the row as a `Value`
                    // A value this chunk's lookup holds as scanned text is validated
                    let known = |field: usize, key: &str| {
                        let held = &lookup.held[lookup.position(&spec[field].0, key)?];
                        Some((held.text.as_deref()?, held.nesting))
                    };
                    if let Some(Ok(scanned)) = scan_rows.then(|| scan::scan_known(s, spec, &known))
                    {
                        for removed in &scanned.removed {
                            let (field, key_field) = &spec[removed.field];
                            let text = &s[removed.value.clone()];
                            let nesting = removed.nesting;
                            lookup.insert_text(field, &removed.key, text, nesting, key_field)?;
                        }
                        return Ok(Some(scanned.slim));
                    }
                    let Ok(mut v) = serde_json::from_str::<Value>(s) else {
                        return Ok(Some(s.to_string()));
                    };
                    strip(&mut v, spec, &mut lookup)?;
                    Ok(Some(serde_json::to_string(&v).unwrap()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok((slim, lookup))
        })
        .collect();

    let mut slim_rows = Vec::new();
    let mut lookup = Lookup::default();
    for chunk in chunks {
        let (rows, part) = chunk?;
        slim_rows.extend(rows);
        for ((field, key), held) in part.ids.into_iter().zip(part.held) {
            let determinant = spec
                .iter()
                .find(|(f, _)| *f == field)
                .map_or("", |(_, k)| k.as_str());
            lookup.insert_held(&field, &key, held, determinant)?;
        }
    }
    Ok(Extracted {
        rows: slim_rows,
        lookup: lookup.into_entries(),
    })
}

#[cfg(test)]
mod tests {
    include!("tests/extract.rs");
}
