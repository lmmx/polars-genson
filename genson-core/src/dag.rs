//! JSON values as a hash-consed DAG: each distinct value is stored once and named by an
//! id, so a schema's repeated parts (a merged schema's map values are mostly repeats) cost
//! one copy, and comparing or copying them costs an id.
//!
//! Ids name values as `Value` writes them, key order included. Equality as `Value` has it,
//! which ignores the order of an object's keys, is `Dag::same`.

use rustc_hash::FxHashMap;
use serde_json::{Map, Number, Value};

/// A value in a `Dag`.
pub(crate) type Id = u32;
/// A string in a `Dag`: an object's key or a string value.
pub(crate) type Sym = u32;

const NULL: u32 = 0;
const FALSE: u32 = 1;
const TRUE: u32 = 2;
const NUMBER: u32 = 3;
const STRING: u32 = 4;
const ARRAY: u32 = 5;
const OBJECT: u32 = 6;

/// Objects with more keys than this are looked up by an index rather than by a scan.
const INDEXED_MIN: usize = 12;
/// Not yet computed, in `Dag::canonical`.
const UNSET: Id = Id::MAX;

/// A value as read from a `Dag`.
#[derive(Clone, Copy)]
pub(crate) enum Term<'a> {
    Null,
    Bool(bool),
    Number(&'a Number),
    String(Sym),
    Array(&'a [Id]),
    Object(Obj<'a>),
}

/// An object's entries, in order.
#[derive(Clone, Copy)]
pub(crate) struct Obj<'a>(&'a [u32]);

impl<'a> Obj<'a> {
    pub(crate) fn len(self) -> usize {
        self.0.len() / 2
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = (Sym, Id)> + 'a {
        self.0.chunks_exact(2).map(|pair| (pair[0], pair[1]))
    }

    pub(crate) fn keys(self) -> impl Iterator<Item = Sym> + 'a {
        self.0.chunks_exact(2).map(|pair| pair[0])
    }

    pub(crate) fn values(self) -> impl Iterator<Item = Id> + 'a {
        self.0.chunks_exact(2).map(|pair| pair[1])
    }

    pub(crate) fn first(self) -> Option<(Sym, Id)> {
        self.iter().next()
    }

    pub(crate) fn to_vec(self) -> Vec<(Sym, Id)> {
        self.iter().collect()
    }
}

/// The keywords and type names schemas are read by, interned first by every `Dag`.
pub(crate) mod kw {
    use super::Sym;
    pub(crate) const TYPE: Sym = 0;
    pub(crate) const PROPERTIES: Sym = 1;
    pub(crate) const REQUIRED: Sym = 2;
    pub(crate) const ITEMS: Sym = 3;
    pub(crate) const ANY_OF: Sym = 4;
    pub(crate) const ADDITIONAL_PROPERTIES: Sym = 5;
    pub(crate) const PATTERN_PROPERTIES: Sym = 6;
    pub(crate) const SCHEMA: Sym = 7;
    pub(crate) const NAMESPACE: Sym = 8;
    pub(crate) const NAME: Sym = 9;
    pub(crate) const NULL: Sym = 10;
    pub(crate) const OBJECT: Sym = 11;
    pub(crate) const ARRAY: Sym = 12;
    pub(crate) const STRING: Sym = 13;
    pub(crate) const NUMBER: Sym = 14;
    pub(crate) const INTEGER: Sym = 15;
    pub(crate) const BOOLEAN: Sym = 16;
    pub(crate) const EMPTY: Sym = 17;
    pub(crate) const WORDS: [&str; 18] = [
        "type",
        "properties",
        "required",
        "items",
        "anyOf",
        "additionalProperties",
        "patternProperties",
        "$schema",
        "namespace",
        "name",
        "null",
        "object",
        "array",
        "string",
        "number",
        "integer",
        "boolean",
        "",
    ];
}

/// Hash-consed JSON values.
pub(crate) struct Dag {
    /// Each value's encoding: its tag, then a string's symbol, a number's index, an array's
    /// items or an object's keys and values in turn.
    terms: Vec<Box<[u32]>>,
    ids: FxHashMap<Box<[u32]>, Id>,
    names: Vec<Box<str>>,
    syms: FxHashMap<Box<str>, Sym>,
    numbers: Vec<Number>,
    number_ids: FxHashMap<String, u32>,
    /// The index of each large object's keys
    indexes: FxHashMap<Id, FxHashMap<Sym, Id>>,
    /// Each value with its objects' keys sorted (`UNSET` where not yet computed)
    canonical: Vec<Id>,
    scratch: Vec<u32>,
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

impl Dag {
    pub(crate) fn new() -> Self {
        let mut dag = Dag {
            terms: Vec::new(),
            ids: FxHashMap::default(),
            names: Vec::new(),
            syms: FxHashMap::default(),
            numbers: Vec::new(),
            number_ids: FxHashMap::default(),
            indexes: FxHashMap::default(),
            canonical: Vec::new(),
            scratch: Vec::new(),
        };
        for word in kw::WORDS {
            dag.sym(word);
        }
        dag
    }

    /// The symbol of a string.
    pub(crate) fn sym(&mut self, s: &str) -> Sym {
        if let Some(&sym) = self.syms.get(s) {
            return sym;
        }
        let sym = self.names.len() as Sym;
        self.names.push(s.into());
        self.syms.insert(s.into(), sym);
        sym
    }

    /// The string of a symbol.
    pub(crate) fn name(&self, sym: Sym) -> &str {
        &self.names[sym as usize]
    }

    fn intern_scratch(&mut self) -> Id {
        if let Some(&id) = self.ids.get(self.scratch.as_slice()) {
            return id;
        }
        let id = self.terms.len() as Id;
        let term: Box<[u32]> = self.scratch.as_slice().into();
        if term[0] == OBJECT && term.len() / 2 > INDEXED_MIN {
            let index = term[1..].chunks_exact(2).map(|p| (p[0], p[1])).collect();
            self.indexes.insert(id, index);
        }
        self.ids.insert(term.clone(), id);
        self.terms.push(term);
        self.canonical.push(UNSET);
        id
    }

    fn intern_tagged(&mut self, tag: u32, payload: impl IntoIterator<Item = u32>) -> Id {
        self.scratch.clear();
        self.scratch.push(tag);
        self.scratch.extend(payload);
        self.intern_scratch()
    }

    pub(crate) fn null(&mut self) -> Id {
        self.intern_tagged(NULL, [])
    }

    pub(crate) fn bool(&mut self, b: bool) -> Id {
        self.intern_tagged(if b { TRUE } else { FALSE }, [])
    }

    pub(crate) fn number(&mut self, n: &Number) -> Id {
        let repr = n.to_string();
        let index = match self.number_ids.get(&repr) {
            Some(&index) => index,
            None => {
                let index = self.numbers.len() as u32;
                self.numbers.push(n.clone());
                self.number_ids.insert(repr, index);
                index
            }
        };
        self.intern_tagged(NUMBER, [index])
    }

    pub(crate) fn string(&mut self, sym: Sym) -> Id {
        self.intern_tagged(STRING, [sym])
    }

    pub(crate) fn str(&mut self, s: &str) -> Id {
        let sym = self.sym(s);
        self.string(sym)
    }

    pub(crate) fn array(&mut self, items: &[Id]) -> Id {
        self.intern_tagged(ARRAY, items.iter().copied())
    }

    pub(crate) fn object(&mut self, entries: &[(Sym, Id)]) -> Id {
        self.intern_tagged(OBJECT, entries.iter().flat_map(|&(k, v)| [k, v]))
    }

    /// The value an id names.
    pub(crate) fn term(&self, id: Id) -> Term<'_> {
        let term = &self.terms[id as usize];
        match term[0] {
            NULL => Term::Null,
            FALSE => Term::Bool(false),
            TRUE => Term::Bool(true),
            NUMBER => Term::Number(&self.numbers[term[1] as usize]),
            STRING => Term::String(term[1]),
            ARRAY => Term::Array(&term[1..]),
            _ => Term::Object(Obj(&term[1..])),
        }
    }

    pub(crate) fn as_object(&self, id: Id) -> Option<Obj<'_>> {
        match self.term(id) {
            Term::Object(obj) => Some(obj),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self, id: Id) -> Option<&[Id]> {
        match self.term(id) {
            Term::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(crate) fn as_sym(&self, id: Id) -> Option<Sym> {
        match self.term(id) {
            Term::String(sym) => Some(sym),
            _ => None,
        }
    }

    /// Whether a value is the string `sym`.
    pub(crate) fn is_sym(&self, id: Id, sym: Sym) -> bool {
        self.as_sym(id) == Some(sym)
    }

    /// An object's value for a key, as `Value::get` (none for anything but an object).
    pub(crate) fn get(&self, id: Id, key: Sym) -> Option<Id> {
        let term = &self.terms[id as usize];
        if term[0] != OBJECT {
            return None;
        }
        if term.len() / 2 > INDEXED_MIN {
            return self.indexes[&id].get(&key).copied();
        }
        term[1..]
            .chunks_exact(2)
            .find(|pair| pair[0] == key)
            .map(|pair| pair[1])
    }

    /// A `Value` as a DAG.
    pub(crate) fn intern_value(&mut self, value: &Value) -> Id {
        match value {
            Value::Null => self.null(),
            Value::Bool(b) => self.bool(*b),
            Value::Number(n) => self.number(n),
            Value::String(s) => self.str(s),
            Value::Array(items) => {
                let items: Vec<Id> = items.iter().map(|item| self.intern_value(item)).collect();
                self.array(&items)
            }
            Value::Object(map) => {
                let entries: Vec<(Sym, Id)> = map
                    .iter()
                    .map(|(k, v)| (self.sym(k), self.intern_value(v)))
                    .collect();
                self.object(&entries)
            }
        }
    }

    /// The `Value` an id names.
    pub(crate) fn to_value(&self, id: Id) -> Value {
        match self.term(id) {
            Term::Null => Value::Null,
            Term::Bool(b) => Value::Bool(b),
            Term::Number(n) => Value::Number(n.clone()),
            Term::String(sym) => Value::String(self.name(sym).to_string()),
            Term::Array(items) => Value::Array(items.iter().map(|&i| self.to_value(i)).collect()),
            Term::Object(obj) => {
                let mut map = Map::with_capacity(obj.len());
                for (k, v) in obj.iter() {
                    map.insert(self.name(k).to_string(), self.to_value(v));
                }
                Value::Object(map)
            }
        }
    }

    /// A value with its objects' keys sorted (and its numbers the first equal one), so
    /// that two values are equal as `Value`s when these are the same id.
    fn canonical(&mut self, id: Id) -> Id {
        let known = self.canonical[id as usize];
        if known != UNSET {
            return known;
        }
        let canonical = match self.term(id) {
            Term::Null | Term::Bool(_) | Term::String(_) => id,
            Term::Number(n) => {
                let first = self.numbers.iter().position(|m| m == n).unwrap() as u32;
                self.intern_tagged(NUMBER, [first])
            }
            Term::Array(items) => {
                let items = items.to_vec();
                let items: Vec<Id> = items.into_iter().map(|i| self.canonical(i)).collect();
                self.array(&items)
            }
            Term::Object(obj) => {
                let mut entries = obj.to_vec();
                for entry in &mut entries {
                    entry.1 = self.canonical(entry.1);
                }
                entries.sort_unstable_by_key(|&(k, _)| k);
                self.object(&entries)
            }
        };
        self.canonical[id as usize] = canonical;
        self.canonical[canonical as usize] = canonical;
        canonical
    }

    /// Whether two values are equal as `Value`s are: objects whatever their keys' order.
    pub(crate) fn same(&mut self, a: Id, b: Id) -> bool {
        a == b || self.canonical(a) == self.canonical(b)
    }
}

/// An object being built, with `Map`'s insertion semantics: a key inserted again keeps its
/// place, and a key removed shifts the later ones.
#[derive(Clone, Default)]
pub(crate) struct ObjBuf(pub(crate) Vec<(Sym, Id)>);

impl ObjBuf {
    pub(crate) fn of(obj: Obj<'_>) -> Self {
        ObjBuf(obj.to_vec())
    }

    pub(crate) fn get(&self, key: Sym) -> Option<Id> {
        self.0.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
    }

    pub(crate) fn contains(&self, key: Sym) -> bool {
        self.0.iter().any(|(k, _)| *k == key)
    }

    /// `Map::insert`: in place when the key is there, else at the end.
    pub(crate) fn insert(&mut self, key: Sym, value: Id) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key, value)),
        }
    }

    /// `Map::shift_remove`.
    pub(crate) fn shift_remove(&mut self, key: Sym) {
        if let Some(at) = self.0.iter().position(|(k, _)| *k == key) {
            self.0.remove(at);
        }
    }

    pub(crate) fn build(&self, dag: &mut Dag) -> Id {
        dag.object(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_values_round_trip_and_share() {
        let mut dag = Dag::new();
        let value = json!({"b": [1, 2.5, -3, "x", null, true], "a": {"c": {}}, "d": []});
        let id = dag.intern_value(&value);
        assert_eq!(dag.to_value(id).to_string(), value.to_string());
        assert_eq!(dag.intern_value(&value), id);
        let reordered = json!({"a": {"c": {}}, "d": [], "b": [1, 2.5, -3, "x", null, true]});
        let other = dag.intern_value(&reordered);
        assert_ne!(other, id);
        assert!(dag.same(id, other));
        let different = dag.intern_value(&json!({"a": {"c": {}}, "d": [], "b": [1]}));
        assert!(!dag.same(id, different));
        // A large object, looked up by its index
        let wide: Map<String, Value> = (0..40).map(|i| (format!("k{i}"), json!(i))).collect();
        let wide = dag.intern_value(&Value::Object(wide));
        let k7 = dag.sym("k7");
        assert_eq!(dag.to_value(dag.get(wide, k7).unwrap()), json!(7));
        let missing = dag.sym("missing");
        assert_eq!(dag.get(wide, missing), None);
    }
}
