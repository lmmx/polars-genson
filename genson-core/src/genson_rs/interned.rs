//! The schemas of rows as interned states: each distinct subtree a row's schema holds is
//! stored once, and the rows' trees share them. A row's state is what its own builder
//! holds after `add_object` (schemas made only from values, so without keywords, and
//! with only the object, list and scalar strategies), built from the row's token tape
//! without allocating a node per value.
//!
//! `add_object` of a value on a node gives the node joined with the value's own state
//! (`join`): properties and strategies in the order they are first seen, `required`
//! intersected, `integer` widened to `number`. That join is memoised, and joining a state
//! with itself is the state, so the items of a long list cost one lookup each.
//!
//! Merging rows into the inference builder (`SchemaNode::absorb_interned`) reads these
//! states, as `absorb` reads the rows' builders. Absorbing a state into a node again,
//! after anything else, leaves the node as it is, so a node skips the states it has
//! absorbed (known by their schema hash, which covers all `absorb` reads): a subtree the
//! rows repeat at a node is merged there once.

use crate::tape::{lex, unescape, Tok};
use rustc_hash::FxHashMap;
use std::borrow::Cow;

/// A state: a node's strategies.
pub(crate) type StateId = u32;
/// A property name.
pub(crate) type KeyId = u32;
/// A row's state and schema hash, or `None` for a row inference skips.
pub(crate) type RowState = Option<(StateId, u64)>;

/// A strategy of a row's node, as `add_object` creates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Strat {
    /// An object, by its interned properties and `required`
    Object(u32),
    /// A list, by its items' state
    List(StateId),
    Null,
    Boolean,
    /// A number, `number` once a float was seen (else `integer`)
    Number(bool),
    String,
}

impl Strat {
    fn same_kind(self, other: Strat) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// An object's properties, in the order first seen, and the indices of those every object
/// held (`required`), ascending.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Obj {
    pub(crate) props: Box<[(KeyId, StateId)]>,
    pub(crate) required: Box<[u32]>,
}

/// Something these states do not reproduce exactly: the rows take the usual path.
pub(crate) struct Unsupported;

/// The empty state: a node no value was added to (the items of an empty list).
const EMPTY: StateId = 0;
const NULL: StateId = 1;
const BOOLEAN: StateId = 2;
const INTEGER: StateId = 3;
const FLOAT: StateId = 4;
const STRING: StateId = 5;

/// The length from which `ListStrategy::add_object` merges a list's items in parallel.
const LONG_LIST: usize = 10;

/// The states of some rows, each distinct one stored once.
pub(crate) struct RowStates {
    keys: Vec<Box<str>>,
    key_ids: FxHashMap<Box<str>, KeyId>,
    states: Vec<Box<[Strat]>>,
    state_ids: FxHashMap<Box<[Strat]>, StateId>,
    objs: Vec<Obj>,
    obj_ids: FxHashMap<Obj, u32>,
    /// The state of an object as a row holds it (every property required), by its
    /// properties: found again without building its `Obj`
    row_objects: FxHashMap<Box<[(KeyId, StateId)]>, StateId>,
    /// The state of a list, by its items' state
    lists: FxHashMap<StateId, StateId>,
    joins: FxHashMap<(StateId, StateId), StateId>,
    /// Each state's schema hash, once computed (`hash`)
    hashes: Vec<Option<u64>>,
    /// Whether each state's tree holds a node of both an object and a list, once found
    mixed: Vec<Option<bool>>,
    key_hashes: Vec<u64>,
    /// Scratch: an object's members while it is read
    members: Vec<(KeyId, StateId)>,
    /// The previous row's keys and objects (row objects' states), by their position in it:
    /// a row shaped as the one before finds them by comparing rather than by hash
    last_keys: Vec<KeyId>,
    last_objects: Vec<StateId>,
    /// The position in the row of the next key and of the next object to end
    key_at: usize,
    object_at: usize,
}

impl RowStates {
    pub(crate) fn new() -> Self {
        let mut states = RowStates {
            keys: Vec::new(),
            key_ids: FxHashMap::default(),
            states: Vec::new(),
            state_ids: FxHashMap::default(),
            objs: Vec::new(),
            obj_ids: FxHashMap::default(),
            row_objects: FxHashMap::default(),
            lists: FxHashMap::default(),
            joins: FxHashMap::default(),
            hashes: Vec::new(),
            mixed: Vec::new(),
            key_hashes: Vec::new(),
            members: Vec::new(),
            last_keys: Vec::new(),
            last_objects: Vec::new(),
            key_at: 0,
            object_at: 0,
        };
        for strats in [
            &[][..],
            &[Strat::Null],
            &[Strat::Boolean],
            &[Strat::Number(false)],
            &[Strat::Number(true)],
            &[Strat::String],
        ] {
            states.state(strats);
        }
        states
    }

    pub(crate) fn strats(&self, state: StateId) -> &[Strat] {
        &self.states[state as usize]
    }

    pub(crate) fn obj(&self, obj: u32) -> &Obj {
        &self.objs[obj as usize]
    }

    pub(crate) fn key(&self, key: KeyId) -> &str {
        &self.keys[key as usize]
    }

    /// The state's identity among all the rows' blocks: its schema hash. `absorb` reads
    /// no more of a node than that hash does (its strategies in schema order, an object's
    /// properties in order and its `required`), so states that hash equal absorb alike.
    pub(crate) fn identity(&self, state: StateId) -> u64 {
        self.hashes[state as usize].expect("a row's states are hashed")
    }

    fn state(&mut self, strats: &[Strat]) -> StateId {
        if let Some(&id) = self.state_ids.get(strats) {
            return id;
        }
        let id = self.states.len() as StateId;
        self.states.push(strats.into());
        self.hashes.push(None);
        self.mixed.push(None);
        self.state_ids.insert(strats.into(), id);
        id
    }

    fn object(&mut self, obj: Obj) -> StateId {
        let id = match self.obj_ids.get(&obj) {
            Some(&id) => id,
            None => {
                let id = self.objs.len() as u32;
                self.objs.push(obj.clone());
                self.obj_ids.insert(obj, id);
                id
            }
        };
        self.state(&[Strat::Object(id)])
    }

    /// The state of an object a row holds, of these properties, all of them required.
    fn row_object(&mut self, first: usize) -> StateId {
        let at = self.object_at;
        self.object_at += 1;
        // The previous row's object here, if it has these properties
        if let Some(&state) = self.last_objects.get(at) {
            let [Strat::Object(obj)] = *self.strats(state) else {
                unreachable!("a row object's state is its object")
            };
            if *self.objs[obj as usize].props == self.members[first..] {
                self.members.truncate(first);
                return state;
            }
        }
        let state = self.new_row_object(first);
        match self.last_objects.get_mut(at) {
            Some(last) => *last = state,
            None => self.last_objects.push(state),
        }
        state
    }

    fn new_row_object(&mut self, first: usize) -> StateId {
        if let Some(&state) = self.row_objects.get(&self.members[first..]) {
            self.members.truncate(first);
            return state;
        }
        let props: Box<[(KeyId, StateId)]> = self.members[first..].into();
        self.members.truncate(first);
        let required = (0..props.len() as u32).collect();
        let state = self.object(Obj {
            props: props.clone(),
            required,
        });
        self.row_objects.insert(props, state);
        state
    }

    /// The id of a row's next key: the previous row's key at its position, if it is the
    /// same text.
    fn row_key(&mut self, key: &str) -> KeyId {
        let at = self.key_at;
        self.key_at += 1;
        if let Some(&last) = self.last_keys.get(at) {
            if *self.keys[last as usize] == *key {
                return last;
            }
        }
        let id = self.key_id(key);
        match self.last_keys.get_mut(at) {
            Some(last) => *last = id,
            None => self.last_keys.push(id),
        }
        id
    }

    fn key_id(&mut self, key: &str) -> KeyId {
        if let Some(&id) = self.key_ids.get(key) {
            return id;
        }
        let id = self.keys.len() as KeyId;
        self.keys.push(key.into());
        self.key_hashes
            .push(xxhash_rust::xxh64::xxh64(key.as_bytes(), 0));
        self.key_ids.insert(key.into(), id);
        id
    }

    /// The state of a node that had `a`'s values added and then `b`'s: `add_object` of
    /// `b`'s values on `a`.
    fn join(&mut self, a: StateId, b: StateId) -> StateId {
        if a == b || b == EMPTY {
            return a;
        }
        if a == EMPTY {
            return b;
        }
        if let Some(&joined) = self.joins.get(&(a, b)) {
            return joined;
        }
        let mut strats: Vec<Strat> = self.strats(a).to_vec();
        for i in 0..self.strats(b).len() {
            let other = self.strats(b)[i];
            match strats.iter().position(|s| s.same_kind(other)) {
                Some(at) => strats[at] = self.join_strat(strats[at], other),
                None => strats.push(other),
            }
        }
        let joined = self.state(&strats);
        self.joins.insert((a, b), joined);
        joined
    }

    fn join_strat(&mut self, a: Strat, b: Strat) -> Strat {
        match (a, b) {
            (Strat::Object(a), Strat::Object(b)) if a != b => {
                let (oa, ob) = (&self.objs[a as usize], &self.objs[b as usize]);
                let mut props = oa.props.to_vec();
                let b_props = ob.props.clone();
                // Where each of `b`'s properties is in `props`, if it is: `required` is where
                // every object of both held the key
                let at: Vec<Option<usize>> = if props.len() > 16 {
                    let index: FxHashMap<KeyId, usize> = props
                        .iter()
                        .enumerate()
                        .map(|(i, &(k, _))| (k, i))
                        .collect();
                    b_props.iter().map(|(k, _)| index.get(k).copied()).collect()
                } else {
                    b_props
                        .iter()
                        .map(|&(key, _)| props.iter().position(|&(k, _)| k == key))
                        .collect()
                };
                let mut required_in_b = vec![false; props.len()];
                for &i in ob.required.iter() {
                    if let Some(p) = at[i as usize] {
                        required_in_b[p] = true;
                    }
                }
                let required: Box<[u32]> = oa
                    .required
                    .iter()
                    .copied()
                    .filter(|&i| required_in_b[i as usize])
                    .collect();
                for (i, &(key, state)) in b_props.iter().enumerate() {
                    match at[i] {
                        Some(p) => props[p].1 = self.join(props[p].1, state),
                        None => props.push((key, state)),
                    }
                }
                let obj = Obj {
                    props: props.into(),
                    required,
                };
                let id = match self.obj_ids.get(&obj) {
                    Some(&id) => id,
                    None => {
                        let id = self.objs.len() as u32;
                        self.objs.push(obj.clone());
                        self.obj_ids.insert(obj, id);
                        id
                    }
                };
                Strat::Object(id)
            }
            (Strat::List(a), Strat::List(b)) => Strat::List(self.join(a, b)),
            (Strat::Number(a), Strat::Number(b)) => Strat::Number(a || b),
            (a, _) => a,
        }
    }

    /// The state of a row's builder, and its schema hash, or `None` for a row inference
    /// skips (nothing but whitespace). `Unsupported` where the usual path might read the
    /// row differently: a row with more than one line or that is not an object (without
    /// `wrap_root`), text `serde_json` rejects as a `Value`, numbers simd-json rejects, and
    /// a long list whose items hold a node of both an object and a list.
    pub(crate) fn row(
        &mut self,
        row: &str,
        ndjson: bool,
        wrap_root: Option<&str>,
        toks: &mut Vec<Tok>,
    ) -> Result<RowState, Unsupported> {
        if row.trim().is_empty() {
            return Ok(None);
        }
        let text = if ndjson {
            // Each line is a document of its own there
            if row.contains('\n') {
                return Err(Unsupported);
            }
            row.trim()
        } else {
            row
        };
        lex(text, toks).map_err(|_| Unsupported)?;
        let is_object = matches!(toks[0], Tok::Obj { .. });
        match wrap_root {
            None if !is_object => return Err(Unsupported),
            // Without a delimiter the usual path splits the text where its braces close, a
            // brace in the wrapping key counted too
            Some(field) if !ndjson && field.contains(['{', '}']) => return Err(Unsupported),
            _ => {}
        }
        if !ndjson && !read_whole(text, wrap_root.is_some()) {
            return Err(Unsupported);
        }
        (self.key_at, self.object_at) = (0, 0);
        let mut state = self.value(text, toks, 0)?;
        if let Some(field) = wrap_root {
            let key = self.key_id(field);
            state = self.object(Obj {
                props: Box::new([(key, state)]),
                required: Box::new([0]),
            });
        }
        Ok(Some((state, self.hash(state))))
    }

    /// The state of the value at token `i`.
    fn value(&mut self, text: &str, toks: &[Tok], i: usize) -> Result<StateId, Unsupported> {
        Ok(match toks[i] {
            Tok::Null => NULL,
            Tok::Bool(_) => BOOLEAN,
            Tok::Str(..) => STRING,
            Tok::Num(s, e) => number_kind(&text[s as usize..e as usize])?,
            Tok::Arr { len, .. } => {
                let mut items = EMPTY;
                let mut j = i + 1;
                for _ in 0..len {
                    let item = self.value(text, toks, j)?;
                    items = self.join(items, item);
                    j = after(toks, j);
                }
                // `ListStrategy::add_object` merges the items of a list this long in parallel
                // pieces by `absorb`, which puts a piece's bare objects after its lists: where
                // a node of the items holds both, the order depends on where rayon split
                if len as usize >= LONG_LIST && self.mixed(items) {
                    return Err(Unsupported);
                }
                match self.lists.get(&items) {
                    Some(&state) => state,
                    None => {
                        let state = self.state(&[Strat::List(items)]);
                        self.lists.insert(items, state);
                        state
                    }
                }
            }
            Tok::Obj { len, .. } => {
                let first = self.members.len();
                // A wide object's keys are found again by hash, a narrow one's in turn
                let mut positions: Option<FxHashMap<KeyId, usize>> = (len > 16)
                    .then(|| FxHashMap::with_capacity_and_hasher(len as usize, Default::default()));
                let mut j = i + 1;
                for _ in 0..len {
                    let Tok::Str(s, e, escaped) = toks[j] else {
                        unreachable!("a key is a string")
                    };
                    let key = if escaped {
                        Cow::Owned(unescape(&text[s as usize..e as usize]))
                    } else {
                        Cow::Borrowed(&text[s as usize..e as usize])
                    };
                    let key = self.row_key(&key);
                    let value = self.value(text, toks, j + 1)?;
                    // A key given again adds its value to the same property
                    let seen = match &mut positions {
                        Some(positions) => {
                            let next = self.members.len();
                            match positions.entry(key) {
                                std::collections::hash_map::Entry::Occupied(at) => Some(*at.get()),
                                std::collections::hash_map::Entry::Vacant(slot) => {
                                    slot.insert(next);
                                    None
                                }
                            }
                        }
                        None => self.members[first..]
                            .iter()
                            .position(|&(k, _)| k == key)
                            .map(|at| first + at),
                    };
                    match seen {
                        Some(at) => self.members[at].1 = self.join(self.members[at].1, value),
                        None => self.members.push((key, value)),
                    }
                    j = after(toks, j + 1);
                }
                self.row_object(first)
            }
        })
    }

    /// Whether a node of the state's tree holds both an object and a list.
    fn mixed(&mut self, state: StateId) -> bool {
        if let Some(mixed) = self.mixed[state as usize] {
            return mixed;
        }
        let strats = self.strats(state).to_vec();
        let mut mixed = strats.iter().any(|s| matches!(s, Strat::Object(_)))
            && strats.iter().any(|s| matches!(s, Strat::List(_)));
        for s in strats {
            if mixed {
                break;
            }
            mixed = match s {
                Strat::Object(o) => {
                    let props = self.objs[o as usize].props.clone();
                    props.iter().any(|&(_, child)| self.mixed(child))
                }
                Strat::List(items) => self.mixed(items),
                _ => false,
            };
        }
        self.mixed[state as usize] = Some(mixed);
        mixed
    }

    /// The state's schema hash: equal for states whose schemas `hash_schema` hashes equal
    /// (the strategies without more than a type in sorted order, after the others in
    /// theirs; an object's properties in order, with their states' hashes, and its sorted
    /// `required`), computed once per state from its parts' hashes.
    pub(crate) fn hash(&mut self, state: StateId) -> u64 {
        if let Some(h) = self.hashes[state as usize] {
            return h;
        }
        let strats = self.strats(state).to_vec();
        let mut parts: Vec<u64> = vec![0x4e4f4445];
        let mut bare: Vec<&'static str> = Vec::new();
        for s in strats {
            match s {
                Strat::Object(o) if !self.objs[o as usize].props.is_empty() => {
                    let h = self.hash_obj(o);
                    parts.push(h);
                }
                Strat::List(items) => {
                    let h = self.hash(items);
                    parts.extend([0x4c495354, h]);
                }
                s => bare.push(bare_type(s)),
            }
        }
        bare.sort_unstable();
        parts.push(0x42415245);
        parts.extend(
            bare.iter()
                .map(|t| xxhash_rust::xxh64::xxh64(t.as_bytes(), 1)),
        );
        let h = hash_words(&parts);
        self.hashes[state as usize] = Some(h);
        h
    }

    fn hash_obj(&mut self, o: u32) -> u64 {
        let obj = self.objs[o as usize].clone();
        let mut parts: Vec<u64> = vec![0x4f424a, obj.props.len() as u64];
        for &(key, state) in obj.props.iter() {
            parts.push(self.key_hashes[key as usize]);
            parts.push(self.hash(state));
        }
        let mut required: Vec<&str> = obj
            .required
            .iter()
            .map(|&i| self.key(obj.props[i as usize].0))
            .collect();
        required.sort_unstable();
        parts.push(required.len() as u64);
        parts.extend(
            required
                .iter()
                .map(|k| xxhash_rust::xxh64::xxh64(k.as_bytes(), 0)),
        );
        hash_words(&parts)
    }

    /// The strategies in the order `SchemaNode::schema_order` gives them: those with more
    /// than a type as they stand, then the bare ones by their type's name.
    pub(crate) fn schema_order(&self, strats: &[Strat]) -> Vec<Strat> {
        let (mut bare, mut ordered): (Vec<Strat>, Vec<Strat>) =
            strats.iter().partition(|s| self.is_bare(**s));
        bare.sort_by_key(|s| bare_type(*s));
        ordered.extend(bare);
        ordered
    }

    /// Whether the strategy's schema is nothing but its type.
    pub(crate) fn is_bare(&self, s: Strat) -> bool {
        match s {
            Strat::Object(o) => self.objs[o as usize].props.is_empty(),
            Strat::List(_) => false,
            _ => true,
        }
    }
}

/// The type a bare strategy's schema gives.
fn bare_type(s: Strat) -> &'static str {
    match s {
        Strat::Object(_) => "object",
        Strat::List(_) => "array",
        Strat::Null => "null",
        Strat::Boolean => "boolean",
        Strat::Number(false) => "integer",
        Strat::Number(true) => "number",
        Strat::String => "string",
    }
}

fn hash_words(words: &[u64]) -> u64 {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    xxhash_rust::xxh64::xxh64(&bytes, 0x9e37_79b9)
}

/// The token after the value at `i`.
fn after(toks: &[Tok], i: usize) -> usize {
    match toks[i] {
        Tok::Arr { next, .. } | Tok::Obj { next, .. } => next as usize,
        _ => i + 1,
    }
}

/// A number's state: an integer that fits an `i64` is one, any other number a float (an
/// integer above `i64::MAX` too, as no Avro `long` or Arrow `Int64` holds it). simd-json
/// rejects a negative integer below `i64::MIN`, and floats near the end of `f64`'s range.
/// Whether the usual path, without a delimiter, reads `text` (one object, inside the
/// wrapping object's braces when `wrapped`) as one document. It splits the text after each
/// `}` that brings its count of braces, those inside strings too, back to zero, and reads
/// each piece apart: no `}` before the end may do so.
fn read_whole(text: &str, wrapped: bool) -> bool {
    let text = text.trim_end().as_bytes();
    let mut depth = i32::from(wrapped);
    for i in memchr::memchr2_iter(b'{', b'}', text) {
        if text[i] == b'{' {
            depth += 1;
        } else {
            depth -= 1;
            // The wrapping object's own `}` comes after the text
            if depth == 0 && (wrapped || i + 1 < text.len()) {
                return false;
            }
        }
    }
    true
}

fn number_kind(text: &str) -> Result<StateId, Unsupported> {
    let integer = !text.contains(['.', 'e', 'E']);
    if integer {
        if text.starts_with('-') {
            return text
                .parse::<i64>()
                .map(|_| INTEGER)
                .map_err(|_| Unsupported);
        }
        return Ok(if text.parse::<i64>().is_ok() {
            INTEGER
        } else {
            FLOAT
        });
    }
    match text.parse::<f64>() {
        Ok(f) if f.abs() < 1e308 => Ok(FLOAT),
        _ => Err(Unsupported),
    }
}
