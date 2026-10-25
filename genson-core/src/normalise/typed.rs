//! Normalisation straight into Arrow: each row's text is read once into a token tape,
//! validated by `serde_json`'s rules for a `Value`, and a plan compiled against the Arrow
//! fields walks the tape, appending to Arrow columns. No `Value` is built for a row or for
//! its normalised form, and a member the schema has no field for is validated, never
//! decoded.
//!
//! The arrays are those `values_to_struct_array` gives for the rows `NormalisePlan`
//! normalises, and the pruned values those it records. Where that is not shown to hold here,
//! nothing is decoded (`None`) and the rows take the `Value` path, which also raises the
//! same errors and panics:
//!
//! - a plan for the `entries` map encoding, for no fields at all, or for fields that are not the ones
//!   `avro_record_fields` gives for the pruned schema (a type other than the one the plan's
//!   node decodes to, a field that is not nullable, or a name given twice);
//! - a value that panics there: any in an empty union; and a record pruned by a field after
//!   one that could panic on its value (the `Value` path reaches that field first);
//! - a value in a column of nulls;
//! - more than `arrow-json` decodes in one batch: over `i32::MAX` bytes in a string column
//!   or items in a list column, or (roughly) `u32::MAX` strings and keys in all.
//!
//! Strings and lists are built with the offsets their Arrow types have: 32-bit for `Utf8`
//! and `List`, as `avro_record_fields` gives them, or 64-bit for `LargeUtf8` and
//! `LargeList`, where the fields are those types (as a target schema's are, see
//! `conform::with_target_offsets`). Either holds the same values, so the limits above are
//! the same for both.

use super::plan::{Plan, Record};
use super::{
    make_promoted_scalar_key, MapEncoding, NormaliseConfig, NormalisePlan, PathSegment, PrunedValue,
};
use crate::tape::{lex, unescape, unescape_into, Tok};
use arrow::array::builder::{
    ArrayBuilder, BooleanBuilder, Float64Builder, GenericStringBuilder, Int64Builder,
    LargeStringBuilder, NullBufferBuilder, StringBuilder,
};
use arrow::array::{
    Array, ArrayRef, LargeListArray, ListArray, MapArray, NullArray, OffsetSizeTrait, StructArray,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, FieldRef, Fields};
use arrow::error::ArrowError;
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{Number, Value};
use std::borrow::Cow;
use std::sync::Arc;

/// The suffix of a promoted field's name (`name__type`): the scalars that go to it.
#[derive(Clone, Copy, PartialEq)]
enum Promoted {
    No,
    String,
    Boolean,
    Int,
    Float,
}

impl Promoted {
    /// As `promoted_scalar_matches` reads `name`.
    fn of(name: &str) -> Self {
        if !name.contains("__") {
            return Promoted::No;
        }
        match name.split("__").last().unwrap_or("") {
            "string" => Promoted::String,
            "boolean" => Promoted::Boolean,
            "int" | "integer" | "long" => Promoted::Int,
            "float" | "double" | "number" => Promoted::Float,
            _ => Promoted::No,
        }
    }
}

/// A plan's node compiled against the Arrow type it decodes into.
enum Node<'p> {
    String,
    Int,
    Double,
    Boolean,
    Record(Box<RecordNode<'p>>),
    Array(Box<Node<'p>>),
    Map(Box<Node<'p>>),
    /// A union holding `null` and another branch
    Nullable(Box<Node<'p>>),
    /// `["null"]`: any value is null there, as on the `Value` path
    NullOnly,
    /// `[]`: any value panics on the `Value` path
    Never,
    /// Values kept as they are, in a column of nulls
    Passthrough,
}

struct RecordNode<'p> {
    fields: Vec<FieldNode<'p>>,
    /// The names `prune` names, without repeats: the record is pruned if it holds a value
    /// for one
    pruning: Vec<(&'p str, Promoted)>,
    has_int_field: bool,
    /// The slot of each name, the fields' (by index) and then the pruning names', where an
    /// object's member for it is found
    slots: FxHashMap<&'p str, u32>,
    /// The names by slot
    names: Vec<&'p str>,
    /// The fields that can be pruned, by index, with the fields before each that could panic
    prunable: Vec<(usize, Vec<usize>)>,
}

struct FieldNode<'p> {
    name: &'p str,
    promoted: Promoted,
    node: Node<'p>,
}

impl<'p> Node<'p> {
    /// Whether a null input appends a null alone, as `Column::append_null` does: not for a
    /// record (a record of nulls), nor `[]` (which the `Value` path cannot normalise null
    /// against).
    fn null_is_null(&self) -> bool {
        !matches!(self, Node::Record(_) | Node::Never)
    }

    /// `plan` compiled against `data_type`, if that is the type `avro_to_arrow_type` gives
    /// for the schema node it was compiled from (under `encoding`'s map type), at every depth.
    fn new(plan: &'p Plan<'_>, data_type: &DataType, encoding: MapEncoding) -> Option<Self> {
        Some(match (plan, data_type) {
            (Plan::String, DataType::Utf8 | DataType::LargeUtf8) => Node::String,
            (Plan::Int, DataType::Int64) => Node::Int,
            (Plan::Double, DataType::Float64) => Node::Double,
            (Plan::Boolean, DataType::Boolean) => Node::Boolean,
            (Plan::Passthrough, DataType::Null) => Node::Passthrough,
            (Plan::Record(record), DataType::Struct(fields)) => {
                Node::Record(Box::new(RecordNode::new(record, fields, encoding)?))
            }
            (Plan::Array(items), DataType::List(field) | DataType::LargeList(field))
                if field.is_nullable() =>
            {
                Node::Array(Box::new(Node::new(items, field.data_type(), encoding)?))
            }
            (Plan::Map(values), DataType::List(field) | DataType::LargeList(field))
                if encoding == MapEncoding::KeyValueEntries =>
            {
                Node::Map(Box::new(Self::entry(values, field.data_type(), encoding)?))
            }
            (Plan::Map(values), DataType::Map(field, false))
                if encoding == MapEncoding::Mapping =>
            {
                Node::Map(Box::new(Self::entry(values, field.data_type(), encoding)?))
            }
            (Plan::Nullable(Some(branch)), data_type) => {
                Node::Nullable(Box::new(Node::new(branch, data_type, encoding)?))
            }
            (Plan::Nullable(None), DataType::Null) => Node::NullOnly,
            // A union without null is normalised as its first branch
            (Plan::Union(first), data_type) => match first.first() {
                Some(first) => Node::new(first, data_type, encoding)?,
                None if data_type == &DataType::Null => Node::Never,
                None => return None,
            },
            _ => return None,
        })
    }

    /// A map's values compiled against its entries' type: a key string and a value.
    fn entry(values: &'p Plan<'_>, entry: &DataType, encoding: MapEncoding) -> Option<Self> {
        let DataType::Struct(entry) = entry else {
            return None;
        };
        match &entry.iter().collect::<Vec<_>>()[..] {
            [key, value]
                if key.name() == "key"
                    && matches!(key.data_type(), DataType::Utf8 | DataType::LargeUtf8)
                    && value.name() == "value"
                    && value.is_nullable() =>
            {
                Node::new(values, value.data_type(), encoding)
            }
            _ => None,
        }
    }

    /// Whether normalising some value against the node gives `Outcome::Pruned`.
    fn can_prune(&self) -> bool {
        match self {
            Node::Record(record) => !record.pruning.is_empty() || !record.prunable.is_empty(),
            Node::Nullable(branch) => branch.can_prune(),
            _ => false,
        }
    }

    /// Whether normalising some value against the node panics on the `Value` path.
    fn can_panic(&self) -> bool {
        match self {
            Node::Record(record) => record.fields.iter().any(|f| f.node.can_panic()),
            Node::Array(inner) | Node::Map(inner) | Node::Nullable(inner) => inner.can_panic(),
            Node::Never => true,
            _ => false,
        }
    }

    /// Whether normalising null against the node panics on the `Value` path: a record gives
    /// each of its fields null, and only an empty union fails on it.
    fn panics_on_null(&self) -> bool {
        match self {
            Node::Record(record) => record.fields.iter().any(|f| f.node.panics_on_null()),
            Node::Never => true,
            _ => false,
        }
    }
}

impl<'p> RecordNode<'p> {
    fn new(record: &'p Record<'_>, fields: &Fields, encoding: MapEncoding) -> Option<Self> {
        let plan_fields = record.fields.as_deref().unwrap_or(&[]);
        if plan_fields.len() != fields.len() {
            return None;
        }
        let mut compiled = Vec::with_capacity(fields.len());
        for (plan_field, field) in plan_fields.iter().zip(fields.iter()) {
            if plan_field.name != field.name() || !field.is_nullable() {
                return None;
            }
            compiled.push(FieldNode {
                name: plan_field.name.as_str(),
                promoted: Promoted::of(plan_field.name),
                node: Node::new(&plan_field.plan, field.data_type(), encoding)?,
            });
        }
        let mut pruning: Vec<(&str, Promoted)> = Vec::new();
        for &name in &record.pruning {
            if !pruning.iter().any(|(n, _)| *n == name) {
                pruning.push((name, Promoted::of(name)));
            }
        }
        let names: Vec<&str> = compiled
            .iter()
            .map(|f| f.name)
            .chain(pruning.iter().map(|(n, _)| *n))
            .collect();
        let mut slots = FxHashMap::default();
        for (slot, name) in names.iter().enumerate() {
            // A field's name given twice (pruning names are not fields') is not decoded here
            if slots.insert(*name, slot as u32).is_some() {
                return None;
            }
        }
        let prunable = (0..compiled.len())
            .filter(|&k| compiled[k].node.can_prune())
            .map(|k| {
                (
                    k,
                    (0..k).filter(|&j| compiled[j].node.can_panic()).collect(),
                )
            })
            .collect();
        Some(RecordNode {
            fields: compiled,
            pruning,
            has_int_field: record.has_int_field,
            slots,
            names,
            prunable,
        })
    }
}

/// [`NormalisePlan`] compiled against the Arrow fields its rows decode into: normalises rows
/// of JSON text straight into a struct array.
pub struct ArrowPlan<'p> {
    root: Node<'p>,
    root_type: DataType,
    cfg: &'p NormaliseConfig,
    /// Rows are decoded only where they fit the schema as their own inferred schema would
    /// (see [`NormalisePlan::to_arrow_strict`])
    strict: bool,
}

impl NormalisePlan<'_> {
    /// The plan compiled for decoding its rows into `fields` (those `avro_record_fields`
    /// gives for the schema without the fields `prune` names), or `None` where that is not
    /// done exactly (see [`ArrowPlan::normalise`]).
    pub fn to_arrow(&self, fields: &Fields) -> Option<ArrowPlan<'_>> {
        self.compile(fields, false)
    }

    /// [`Self::to_arrow`], decoding only rows that fit the schema strictly: rows whose
    /// own inferred schema the schema already holds, so that they normalise as they would
    /// against the schema inferred from them. A row with a member the schema has no field
    /// for, a value of a type its node does not hold (a number where a string is, a
    /// fraction where an integer is, a scalar where an array, a map or a record is, but a
    /// scalar a record's promoted field takes), or a value where the schema saw only
    /// nulls, is not decoded: [`ArrowPlan::normalise`] gives `None`. A schema with a union
    /// of two or more types (normalised as its first) is not compiled.
    pub fn to_arrow_strict(&self, fields: &Fields) -> Option<ArrowPlan<'_>> {
        self.compile(fields, true)
    }

    fn compile(&self, fields: &Fields, strict: bool) -> Option<ArrowPlan<'_>> {
        let root_type = DataType::Struct(fields.clone());
        // `values_to_struct_array` fails to build a struct array without fields
        // A value of a union's other branch would infer to another schema than the union
        if !matches!(self.root, Plan::Record(_))
            || (strict && self.has_union)
            || fields.is_empty()
            || self.cfg.map_encoding == MapEncoding::Entries
        {
            return None;
        }
        Some(ArrowPlan {
            root: Node::new(&self.root, &root_type, self.cfg.map_encoding)?,
            root_type,
            cfg: self.cfg,
            strict,
        })
    }
}

/// The value of a slot no member was found for.
const NONE: u32 = u32::MAX;

/// Where a value comes from: a token of the row, nothing (null), or the row wrapped as the
/// one field of an object (`wrap_root`), the row being `None` where its text was invalid.
#[derive(Clone, Copy)]
enum Input<'a> {
    Null,
    Tok(usize),
    Wrapped(&'a str, Option<usize>),
}

/// A step of the path to a value, as `PathSegment` gives it once a value is pruned.
enum Seg<'a, 'r> {
    Field(&'a str),
    Index(usize),
    Key(Cow<'r, str>),
}

/// A string column being built, with 32-bit (`Utf8`) or 64-bit (`LargeUtf8`) offsets.
enum Strings {
    Small(StringBuilder),
    Large(LargeStringBuilder),
}

impl Strings {
    fn new(data_type: &DataType) -> Result<Self, Unsupported> {
        match data_type {
            DataType::Utf8 => Ok(Strings::Small(StringBuilder::new())),
            DataType::LargeUtf8 => Ok(Strings::Large(LargeStringBuilder::new())),
            _ => Err(Unsupported),
        }
    }

    fn append(&mut self, s: &str) -> Result<(), Unsupported> {
        match self {
            Strings::Small(b) => append_str(b, s),
            Strings::Large(b) => append_str(b, s),
        }
    }

    fn append_nulls(&mut self, n: usize) {
        match self {
            Strings::Small(b) => b.append_nulls(n),
            Strings::Large(b) => b.append_nulls(n),
        }
    }

    fn len(&self) -> usize {
        match self {
            Strings::Small(b) => b.len(),
            Strings::Large(b) => b.len(),
        }
    }

    fn finish(self) -> ArrayRef {
        match self {
            Strings::Small(mut b) => Arc::new(b.finish()),
            Strings::Large(mut b) => Arc::new(b.finish()),
        }
    }
}

/// A list's (or map's) offsets, 32-bit (`List`, `Map`) or 64-bit (`LargeList`).
enum Offsets {
    Small(Vec<i32>),
    Large(Vec<i64>),
}

impl Offsets {
    fn new(large: bool) -> Self {
        match large {
            false => Offsets::Small(vec![0]),
            true => Offsets::Large(vec![0]),
        }
    }

    /// The last offset `n` more times: `n` empty or null entries.
    fn repeat_last(&mut self, n: usize) {
        match self {
            Offsets::Small(o) => o.extend(std::iter::repeat_n(*o.last().unwrap(), n)),
            Offsets::Large(o) => o.extend(std::iter::repeat_n(*o.last().unwrap(), n)),
        }
    }

    /// The offset `added` items on, unless past what `arrow-json` decodes into a list or
    /// map (whichever the offsets' width).
    fn push(&mut self, added: usize) -> Result<(), Unsupported> {
        let last = match self {
            Offsets::Small(o) => *o.last().unwrap() as usize,
            Offsets::Large(o) => *o.last().unwrap() as usize,
        };
        let next = i32::try_from(last + added).map_err(|_| Unsupported)?;
        match self {
            Offsets::Small(o) => o.push(next),
            Offsets::Large(o) => o.push(i64::from(next)),
        }
        Ok(())
    }

    /// The entries they bound.
    fn len(&self) -> usize {
        match self {
            Offsets::Small(o) => o.len() - 1,
            Offsets::Large(o) => o.len() - 1,
        }
    }
}

/// A column being built, of one of the Arrow types `avro_to_arrow_type` gives, or one of
/// those with 64-bit offsets.
enum Column {
    Str(Strings),
    Int(Int64Builder),
    Float(Float64Builder),
    Bool(BooleanBuilder),
    Null(usize),
    List {
        field: FieldRef,
        offsets: Offsets,
        valid: NullBufferBuilder,
        items: Box<Column>,
    },
    Struct {
        fields: Fields,
        valid: NullBufferBuilder,
        len: usize,
        children: Vec<Column>,
    },
    Map {
        field: FieldRef,
        /// Always 32-bit: a map's offsets are
        offsets: Offsets,
        valid: NullBufferBuilder,
        keys: Strings,
        values: Box<Column>,
    },
}

/// Something the plan and a column cannot decode exactly as the `Value` path would.
struct Unsupported;

impl Column {
    fn new(data_type: &DataType) -> Result<Self, Unsupported> {
        Ok(match data_type {
            DataType::Utf8 | DataType::LargeUtf8 => Column::Str(Strings::new(data_type)?),
            DataType::Int64 => Column::Int(Int64Builder::new()),
            DataType::Float64 => Column::Float(Float64Builder::new()),
            DataType::Boolean => Column::Bool(BooleanBuilder::new()),
            DataType::Null => Column::Null(0),
            DataType::List(field) | DataType::LargeList(field) => Column::List {
                field: field.clone(),
                offsets: Offsets::new(matches!(data_type, DataType::LargeList(_))),
                valid: NullBufferBuilder::new(0),
                items: Box::new(Column::new(field.data_type())?),
            },
            DataType::Struct(fields) => Column::Struct {
                fields: fields.clone(),
                valid: NullBufferBuilder::new(0),
                len: 0,
                children: fields
                    .iter()
                    .map(|f| Column::new(f.data_type()))
                    .collect::<Result<_, _>>()?,
            },
            DataType::Map(field, false) => {
                let DataType::Struct(entry) = field.data_type() else {
                    return Err(Unsupported);
                };
                let [key, value] = &entry.iter().collect::<Vec<_>>()[..] else {
                    return Err(Unsupported);
                };
                Column::Map {
                    field: field.clone(),
                    offsets: Offsets::new(false),
                    valid: NullBufferBuilder::new(0),
                    keys: Strings::new(key.data_type())?,
                    values: Box::new(Column::new(value.data_type())?),
                }
            }
            _ => return Err(Unsupported),
        })
    }

    /// A null, as decoding a JSON null gives. A struct's children take theirs when next
    /// written to, or when it is finished (`pad`).
    fn append_null(&mut self) {
        self.append_nulls(1);
    }

    /// `n` nulls.
    fn append_nulls(&mut self, n: usize) {
        match self {
            Column::Str(b) => b.append_nulls(n),
            Column::Int(b) => b.append_nulls(n),
            Column::Float(b) => b.append_nulls(n),
            Column::Bool(b) => b.append_nulls(n),
            Column::Null(k) => *k += n,
            Column::List { offsets, valid, .. } | Column::Map { offsets, valid, .. } => {
                offsets.repeat_last(n);
                valid.append_n_nulls(n);
            }
            Column::Struct { valid, len, .. } => {
                valid.append_n_nulls(n);
                *len += n;
            }
        }
    }

    /// The rows the column holds.
    fn len(&self) -> usize {
        match self {
            Column::Str(b) => b.len(),
            Column::Int(b) => b.len(),
            Column::Float(b) => b.len(),
            Column::Bool(b) => b.len(),
            Column::Null(n) => *n,
            Column::List { offsets, .. } | Column::Map { offsets, .. } => offsets.len(),
            Column::Struct { len, .. } => *len,
        }
    }

    /// A struct's child brought to the struct's `rows` with nulls: the rows it was not
    /// written for, its field absent or the struct null, are null.
    fn pad(&mut self, rows: usize) {
        let missing = rows - self.len();
        if missing > 0 {
            self.append_nulls(missing);
        }
    }

    fn finish(self) -> Result<ArrayRef, ArrowError> {
        Ok(match self {
            Column::Str(b) => b.finish(),
            Column::Int(mut b) => Arc::new(b.finish()),
            Column::Float(mut b) => Arc::new(b.finish()),
            Column::Bool(mut b) => Arc::new(b.finish()),
            Column::Null(n) => Arc::new(NullArray::new(n)),
            Column::List {
                field,
                offsets,
                mut valid,
                items,
            } => match offsets {
                Offsets::Small(offsets) => Arc::new(ListArray::try_new(
                    field,
                    OffsetBuffer::new(offsets.into()),
                    items.finish()?,
                    valid.finish(),
                )?),
                Offsets::Large(offsets) => Arc::new(LargeListArray::try_new(
                    field,
                    OffsetBuffer::new(offsets.into()),
                    items.finish()?,
                    valid.finish(),
                )?),
            },
            Column::Struct {
                fields,
                mut valid,
                len,
                children,
            } => Arc::new(Self::finish_struct(fields, valid.finish(), len, children)?),
            Column::Map {
                field,
                offsets,
                mut valid,
                keys,
                values,
            } => {
                let DataType::Struct(entry) = field.data_type() else {
                    unreachable!("a map's entries are a struct")
                };
                let len = keys.len();
                let children = vec![Column::Str(keys), *values];
                let entries = Self::finish_struct(entry.clone(), None, len, children)?;
                let Offsets::Small(offsets) = offsets else {
                    unreachable!("a map's offsets are 32-bit")
                };
                Arc::new(MapArray::try_new(
                    field,
                    OffsetBuffer::new(offsets.into()),
                    entries,
                    valid.finish(),
                    false,
                )?)
            }
        })
    }

    fn finish_struct(
        fields: Fields,
        nulls: Option<NullBuffer>,
        len: usize,
        children: Vec<Column>,
    ) -> Result<StructArray, ArrowError> {
        let children = children
            .into_iter()
            .map(|mut child| {
                child.pad(len);
                child.finish()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if children.is_empty() {
            return Ok(StructArray::new_empty_fields(len, nulls));
        }
        StructArray::try_new(fields, children, nulls)
    }
}

/// `s` appended, unless the column would then hold more than `arrow-json` decodes into one
/// (whichever the offsets' width).
fn append_str<O: OffsetSizeTrait>(
    b: &mut GenericStringBuilder<O>,
    s: &str,
) -> Result<(), Unsupported> {
    if b.values_slice().len() + s.len() > i32::MAX as usize {
        return Err(Unsupported);
    }
    b.append_value(s);
    Ok(())
}

/// A string token (or another value, as its JSON text) appended to a string column.
fn put_str<'t, O: OffsetSizeTrait>(
    b: &mut GenericStringBuilder<O>,
    tok: Option<Tok>,
    text: impl Fn((u32, u32)) -> &'t str,
    other: impl FnOnce() -> String,
) -> Result<(), Unsupported> {
    match tok {
        // Unescaped into the column, no longer than its text
        Some(Tok::Str(s, e, true)) => {
            let raw = text((s, e));
            if b.values_slice().len() + raw.len() > i32::MAX as usize {
                return Err(Unsupported);
            }
            unescape_into(raw, b);
            b.append_value("");
            Ok(())
        }
        Some(Tok::Str(s, e, false)) => append_str(b, text((s, e))),
        _ => append_str(b, &other()),
    }
}

/// Whether a key (by its text, without escapes) is given twice among `members`.
fn has_repeat<'r>(
    row: &'r str,
    toks: &[Tok],
    members: &[(usize, usize)],
    seen: &mut FxHashSet<&'r str>,
) -> bool {
    let key = |&(key, _): &(usize, usize)| match toks[key] {
        Tok::Str(s, e, _) => &row[s as usize..e as usize],
        _ => unreachable!("a key is a string"),
    };
    if members.len() <= 16 {
        return (1..members.len()).any(|i| members[..i].iter().any(|m| key(m) == key(&members[i])));
    }
    seen.clear();
    !members.iter().all(|m| seen.insert(key(m)))
}

/// What appending a value gave: it was appended, or it was emptied by pruning and nothing
/// was appended (the caller appends a null, or leaves it out).
enum Put {
    Kept,
    Emptied,
}

/// The decoding of one batch of rows.
struct Decoder<'a, 'r> {
    cfg: &'a NormaliseConfig,
    /// See [`ArrowPlan::strict`]
    strict: bool,
    /// A member the schema has no field for was found, when strict
    misfit: bool,
    row: &'r str,
    toks: Vec<Tok>,
    row_index: usize,
    path: Vec<Seg<'a, 'r>>,
    pruned: Vec<PrunedValue>,
    /// The members of the maps being decoded (key token, value token), innermost last
    members: Vec<(usize, usize)>,
    /// For each object token of the row, where its record's slots start in `slots`, with
    /// the row they were found for (by `generation`)
    found: Vec<(u32, u32)>,
    generation: u32,
    /// The value token of each record's slot, for the objects found
    slots: Vec<u32>,
    seen: FxHashSet<&'r str>,
    /// The strings and keys `arrow-json` would hold for the normalised rows
    strings: u64,
}

impl<'a, 'r> Decoder<'a, 'r> {
    fn text(&self, (start, end): (u32, u32)) -> &'r str {
        &self.row[start as usize..end as usize]
    }

    /// A string token's content, as `Value` holds it.
    fn str_of(&self, start: u32, end: u32, escaped: bool) -> Cow<'r, str> {
        if escaped {
            Cow::Owned(unescape(self.text((start, end))))
        } else {
            Cow::Borrowed(self.text((start, end)))
        }
    }

    /// A number token as `Value` holds it.
    fn number(&self, start: u32, end: u32) -> Number {
        let text = self.text((start, end));
        // Up to 18 digits, without a sign, fraction or exponent, fit an i64 either way
        let (negative, digits) = match text.strip_prefix('-') {
            Some(digits) => (true, digits),
            None => (false, text),
        };
        if !digits.is_empty()
            && digits.len() <= 18
            && digits.bytes().all(|d| d.is_ascii_digit())
            && (digits.len() == 1 || !digits.starts_with('0'))
            && !(negative && digits == "0")
        {
            let n: i64 = digits.parse().expect("digits");
            return if negative {
                Number::from(-n)
            } else {
                Number::from(n as u64)
            };
        }
        text.parse().expect("validated number")
    }

    /// A value's `Value`, built from its text (for a pruned value, or a string field's
    /// non-string value).
    fn value_of(&self, input: Input<'_>) -> Value {
        match input {
            Input::Null => Value::Null,
            Input::Tok(i) => match self.toks[i] {
                Tok::Null => Value::Null,
                Tok::Bool(b) => Value::Bool(b),
                Tok::Num(s, e) => Value::Number(self.number(s, e)),
                Tok::Str(s, e, escaped) => Value::String(self.str_of(s, e, escaped).into_owned()),
                Tok::Arr { len, .. } => {
                    let mut items = Vec::with_capacity(len as usize);
                    let mut j = i + 1;
                    for _ in 0..len {
                        items.push(self.value_of(Input::Tok(j)));
                        j = self.after(j);
                    }
                    Value::Array(items)
                }
                // A repeated key keeps its first place and takes its last value, as
                // `serde_json` reads it
                Tok::Obj { len, .. } => {
                    let mut members = serde_json::Map::with_capacity(len as usize);
                    let mut j = i + 1;
                    for _ in 0..len {
                        members.insert(
                            self.key_of(j).into_owned(),
                            self.value_of(Input::Tok(j + 1)),
                        );
                        j = self.after(j + 1);
                    }
                    Value::Object(members)
                }
            },
            Input::Wrapped(field, inner) => {
                let inner = inner.map_or(Value::Null, |i| self.value_of(Input::Tok(i)));
                Value::Object(std::iter::once((field.to_string(), inner)).collect())
            }
        }
    }

    fn is_null(&self, input: Input<'_>) -> bool {
        match input {
            Input::Null => true,
            Input::Tok(i) => matches!(self.toks[i], Tok::Null),
            Input::Wrapped(..) => false,
        }
    }

    /// The token after the value at `i`.
    fn after(&self, i: usize) -> usize {
        match self.toks[i] {
            Tok::Arr { next, .. } | Tok::Obj { next, .. } => next as usize,
            _ => i + 1,
        }
    }

    fn key_of(&self, key: usize) -> Cow<'r, str> {
        match self.toks[key] {
            Tok::Str(s, e, escaped) => self.str_of(s, e, escaped),
            _ => unreachable!("a key is a string"),
        }
    }

    /// Where the slots of `record` for the object at `obj` start in `self.slots`: each
    /// slot's member as `Value::get` finds it (the last one of that key), found once a row.
    fn object_slots(&mut self, obj: usize, record: &RecordNode<'_>) -> usize {
        let (generation, base) = self.found[obj];
        if generation == self.generation {
            return base as usize;
        }
        let Tok::Obj { len, .. } = self.toks[obj] else {
            unreachable!("an object")
        };
        let base = self.slots.len();
        self.slots.resize(base + record.names.len(), NONE);
        let mut j = obj + 1;
        // Members mostly come in the order of the schema's fields
        let mut next = 0;
        for _ in 0..len {
            let Tok::Str(s, e, escaped) = self.toks[j] else {
                unreachable!("a key is a string")
            };
            let key = self.str_of(s, e, escaped);
            let slot = match record.names.get(next) {
                Some(name) if *name == key => Some(next as u32),
                _ => record.slots.get(&*key).copied(),
            };
            match slot {
                Some(slot) => {
                    self.slots[base + slot as usize] = j as u32 + 1;
                    next = slot as usize + 1;
                }
                None if self.strict => self.misfit = true,
                None => {}
            }
            j = self.after(j + 1);
        }
        self.found[obj] = (self.generation, base as u32);
        base
    }

    /// The slots of `record` for `input`, where it is an object.
    fn slots_of(&mut self, record: &RecordNode<'_>, input: Input<'_>) -> Option<usize> {
        match input {
            Input::Tok(i) if matches!(self.toks[i], Tok::Obj { .. }) => {
                Some(self.object_slots(i, record))
            }
            _ => None,
        }
    }

    /// The input of a record's field (or pruning name) at `slot` (`field_value`): the
    /// object's member, or a scalar in place of a record that goes to its promoted field.
    fn field_input<'i>(
        &self,
        record: &RecordNode<'_>,
        input: Input<'i>,
        slots: Option<usize>,
        slot: usize,
        promoted: Promoted,
    ) -> Input<'i> {
        match (input, slots) {
            (Input::Tok(_), Some(base)) => match self.slots[base + slot] {
                NONE => Input::Null,
                value => Input::Tok(value as usize),
            },
            (Input::Tok(i), None) if self.promoted_matches(i, promoted, record.has_int_field) => {
                Input::Tok(i)
            }
            (Input::Wrapped(field, Some(i)), _) if field == record.names[slot] => Input::Tok(i),
            _ => Input::Null,
        }
    }

    /// `promoted_scalar_matches` on a token.
    fn promoted_matches(&self, i: usize, promoted: Promoted, has_int_field: bool) -> bool {
        match (self.toks[i], promoted) {
            (Tok::Str(..), Promoted::String) => true,
            (Tok::Bool(_), Promoted::Boolean) => true,
            (Tok::Num(s, e), Promoted::Int) => {
                let n = self.number(s, e);
                n.is_i64() || n.is_u64()
            }
            (Tok::Num(s, e), Promoted::Float) => self.number(s, e).is_f64() || !has_int_field,
            _ => false,
        }
    }

    /// Whether normalising `input` against `node` gives `Outcome::Pruned`: a record holding a
    /// field `prune` names, or with a field that is such a record, through record fields.
    /// `Unsupported` where a field before the one that prunes could panic, as the `Value`
    /// path, which normalises the fields in order, would first.
    fn prunes(&mut self, node: &Node<'_>, input: Input<'_>) -> Result<bool, Unsupported> {
        match node {
            Node::Record(record) => {
                let slots = self.slots_of(record, input);
                let first = record.fields.len();
                for (k, &(_, promoted)) in record.pruning.iter().enumerate() {
                    // `holds`: a value, not null, for the name
                    let held = self.field_input(record, input, slots, first + k, promoted);
                    if !self.is_null(held) {
                        return Ok(true);
                    }
                }
                for (k, could_panic) in &record.prunable {
                    let field = &record.fields[*k];
                    let field_input = self.field_input(record, input, slots, *k, field.promoted);
                    if !self.prunes(&field.node, field_input)? {
                        continue;
                    }
                    // A field before it that holds a value, or that fails on null
                    for &j in could_panic {
                        let before = &record.fields[j];
                        let held = self.field_input(record, input, slots, j, before.promoted);
                        if !self.is_null(held) || before.node.panics_on_null() {
                            return Err(Unsupported);
                        }
                    }
                    return Ok(true);
                }
                Ok(false)
            }
            Node::Nullable(branch) if !self.is_null(input) => self.prunes(branch, input),
            _ => Ok(false),
        }
    }

    /// Whether a value (not null) fits `node` as its inferred schema would (see
    /// [`NormalisePlan::to_arrow_strict`]); a record's members are checked as its slots
    /// are found, and the items of an array or map as they are put.
    fn fits(&self, node: &Node<'_>, input: Input<'_>) -> bool {
        let Input::Tok(i) = input else {
            // The row wrapped as the root record's one field
            return matches!(node, Node::Record(_));
        };
        match (node, self.toks[i]) {
            (Node::String, Tok::Str(..)) => true,
            (Node::Int, Tok::Num(s, e)) => self.number(s, e).is_i64(),
            (Node::Double, Tok::Num(..)) => true,
            (Node::Boolean, Tok::Bool(_)) => true,
            (Node::Record(_), Tok::Obj { .. }) => true,
            (Node::Record(record), Tok::Str(..) | Tok::Num(..) | Tok::Bool(_)) => {
                record.fields.iter().any(|f| {
                    f.promoted != Promoted::No
                        && self.promoted_matches(i, f.promoted, record.has_int_field)
                })
            }
            (Node::Array(_), Tok::Arr { .. }) => true,
            (Node::Map(_), Tok::Obj { .. }) => true,
            (Node::Nullable(branch), _) => self.fits(branch, input),
            _ => false,
        }
    }

    fn record_pruned(&mut self, input: Input<'_>) {
        let value = self.value_of(input);
        let path = self
            .path
            .iter()
            .map(|seg| match seg {
                Seg::Field(name) => PathSegment::Key(name.to_string()),
                Seg::Index(i) => PathSegment::Index(*i),
                Seg::Key(key) => PathSegment::Key(key.to_string()),
            })
            .collect();
        self.pruned.push(PrunedValue {
            row: self.row_index,
            path,
            value,
        });
    }

    /// Normalise `input` against `node` and append it to `col`.
    fn put(
        &mut self,
        node: &'a Node<'a>,
        col: &mut Column,
        input: Input<'a>,
        field_name: Option<&str>,
    ) -> Result<Put, Unsupported> {
        let cfg = self.cfg;
        let tok = match input {
            Input::Tok(i) => Some(self.toks[i]),
            Input::Null | Input::Wrapped(..) => None,
        };
        let null = self.is_null(input);
        // Null where a record is not nullable is a record of nulls, where the schema
        // inferred from the rows would hold it nullable and give null
        if self.strict
            && (if null {
                matches!(node, Node::Record(_))
            } else {
                !self.fits(node, input)
            })
        {
            return Err(Unsupported);
        }
        match (node, col) {
            (Node::String, Column::Str(b)) => {
                if null {
                    b.append_nulls(1);
                    return Ok(Put::Kept);
                }
                let text = |span| self.text(span);
                let other = || self.value_of(input).to_string();
                match b {
                    Strings::Small(b) => put_str(b, tok, text, other)?,
                    Strings::Large(b) => put_str(b, tok, text, other)?,
                }
                self.strings += 1;
            }
            (Node::Int, Column::Int(b)) => match tok {
                Some(Tok::Num(s, e)) => b.append_option(self.number(s, e).as_i64()),
                Some(Tok::Str(s, e, escaped)) if cfg.coerce_string => {
                    b.append_option(self.str_of(s, e, escaped).parse::<i64>().ok())
                }
                _ => b.append_null(),
            },
            (Node::Double, Column::Float(b)) => match tok {
                Some(Tok::Num(s, e)) => b.append_option(self.number(s, e).as_f64()),
                Some(Tok::Str(s, e, escaped)) if cfg.coerce_string => b.append_option(
                    self.str_of(s, e, escaped)
                        .parse::<f64>()
                        .ok()
                        .filter(|f| f.is_finite()),
                ),
                _ => b.append_null(),
            },
            (Node::Boolean, Column::Bool(b)) => match tok {
                Some(Tok::Bool(v)) => b.append_value(v),
                Some(Tok::Str(s, e, escaped)) if cfg.coerce_string => {
                    match &*self.str_of(s, e, escaped) {
                        "true" | "1" => b.append_value(true),
                        "false" | "0" => b.append_value(false),
                        _ => b.append_null(),
                    }
                }
                _ => b.append_null(),
            },
            (
                Node::Record(record),
                Column::Struct {
                    valid,
                    len,
                    children,
                    ..
                },
            ) => {
                // `Node::new` has checked the children are the fields' columns, in order
                let slots = self.slots_of(record, input);
                let row = *len;
                for (k, (field, child)) in record.fields.iter().zip(children.iter_mut()).enumerate()
                {
                    let field_input = self.field_input(record, input, slots, k, field.promoted);
                    // A field null here is a null its child takes when next written to: of
                    // a wide record's fields, most are absent from each row
                    if matches!(field_input, Input::Null) && field.node.null_is_null() {
                        continue;
                    }
                    child.pad(row);
                    self.path.push(Seg::Field(field.name));
                    let put = self.put(&field.node, child, field_input, Some(field.name));
                    self.path.pop();
                    if let Put::Emptied = put? {
                        child.append_null();
                    }
                }
                // Each field's key
                self.strings += record.fields.len() as u64;
                valid.append_non_null();
                *len += 1;
            }
            (
                Node::Array(items),
                Column::List {
                    offsets,
                    valid,
                    items: item_col,
                    ..
                },
            ) => match (tok, input) {
                _ if null => {
                    offsets.repeat_last(1);
                    valid.append_null();
                }
                (Some(Tok::Arr { len, .. }), Input::Tok(i)) => {
                    if len == 0 && cfg.empty_as_null {
                        offsets.repeat_last(1);
                        valid.append_null();
                        return Ok(Put::Kept);
                    }
                    let mut kept = 0;
                    let mut j = i + 1;
                    for index in 0..len as usize {
                        if self.put_item(items, item_col, Input::Tok(j), index, field_name)? {
                            kept += 1;
                        }
                        j = self.after(j);
                    }
                    if kept == 0 && len > 0 {
                        return Ok(Put::Emptied);
                    }
                    offsets.push(kept)?;
                    valid.append_non_null();
                }
                // Any other value is normalised as an array of itself
                _ => {
                    if !self.put_item(items, item_col, input, 0, field_name)? {
                        return Ok(Put::Emptied);
                    }
                    offsets.push(1)?;
                    valid.append_non_null();
                }
            },
            (Node::Map(values), col @ (Column::List { .. } | Column::Map { .. })) => {
                return self.put_map(values, col, input, field_name)
            }
            (Node::Nullable(branch), col) => {
                if !null {
                    return self.put(branch, col, input, field_name);
                }
                col.append_null();
            }
            (Node::NullOnly, col) => col.append_null(),
            (Node::Passthrough, Column::Null(n)) if null => *n += 1,
            _ => return Err(Unsupported),
        }
        Ok(Put::Kept)
    }

    /// An array's item: kept (`true`), or pruned or emptied and left out.
    fn put_item(
        &mut self,
        items: &'a Node<'a>,
        col: &mut Column,
        item: Input<'a>,
        index: usize,
        field_name: Option<&str>,
    ) -> Result<bool, Unsupported> {
        self.path.push(Seg::Index(index));
        let kept = self.put_or_prune(items, col, item, field_name);
        self.path.pop();
        kept
    }

    /// An array's item or a map's value, recorded if it is pruned (`false`), else put.
    fn put_or_prune(
        &mut self,
        node: &'a Node<'a>,
        col: &mut Column,
        item: Input<'a>,
        field_name: Option<&str>,
    ) -> Result<bool, Unsupported> {
        if node.can_prune() && self.prunes(node, item)? {
            self.record_pruned(item);
            return Ok(false);
        }
        Ok(matches!(self.put(node, col, item, field_name)?, Put::Kept))
    }

    /// A map: each entry normalised against `values`, as a key/value list or a map.
    fn put_map(
        &mut self,
        values: &'a Node<'a>,
        col: &mut Column,
        input: Input<'a>,
        field_name: Option<&str>,
    ) -> Result<Put, Unsupported> {
        let cfg = self.cfg;
        // The column's offsets and validity, and its entries' keys and values
        let (offsets, valid, keys, value_col, mut entries) = match col {
            Column::List {
                offsets,
                valid,
                items,
                ..
            } => match &mut **items {
                Column::Struct {
                    valid: entry_valid,
                    len,
                    children,
                    ..
                } => match &mut children[..] {
                    [Column::Str(keys), value_col] => {
                        (offsets, valid, keys, value_col, Some((entry_valid, len)))
                    }
                    _ => return Err(Unsupported),
                },
                _ => return Err(Unsupported),
            },
            Column::Map {
                offsets,
                valid,
                keys,
                values: value_col,
                ..
            } => (offsets, valid, keys, &mut **value_col, None),
            _ => return Err(Unsupported),
        };
        let i = match input {
            Input::Tok(i) if !matches!(self.toks[i], Tok::Null) => i,
            Input::Wrapped(..) => return Err(Unsupported),
            _ => {
                offsets.repeat_last(1);
                valid.append_null();
                return Ok(Put::Kept);
            }
        };
        // The entries as `Value`'s map holds them: each key at its first place, with its last
        // value
        let first = self.members.len();
        let mut wrapped_key = None;
        match self.toks[i] {
            Tok::Obj { len, escaped, .. } => {
                if len == 0 && cfg.empty_as_null {
                    offsets.repeat_last(1);
                    valid.append_null();
                    return Ok(Put::Kept);
                }
                let mut j = i + 1;
                for _ in 0..len {
                    self.members.push((j, j + 1));
                    j = self.after(j + 1);
                }
                if escaped
                    || has_repeat(self.row, &self.toks, &self.members[first..], &mut self.seen)
                {
                    self.dedup_members(first);
                }
            }
            // Scalar fallback: wrap under a promoted key
            tok => {
                let scalar_type = match tok {
                    Tok::Str(..) => "string",
                    Tok::Num(s, e) if self.number(s, e).is_i64() => "int",
                    Tok::Num(..) => "float",
                    Tok::Bool(_) => "boolean",
                    _ => "unknown",
                };
                wrapped_key = Some(make_promoted_scalar_key(
                    field_name.unwrap_or(""),
                    scalar_type,
                ));
                self.members.push((usize::MAX, i));
            }
        }
        let len = self.members.len() - first;
        let mut kept = 0;
        let mut result = Ok(());
        // A kept entry's strings: its key, and in a key/value list the entry's two keys
        let entry_strings = if entries.is_some() { 3 } else { 1 };
        for m in first..self.members.len() {
            let (key_tok, value_tok) = self.members[m];
            let key = match wrapped_key.take() {
                Some(k) => Cow::Owned(k),
                None => self.key_of(key_tok),
            };
            self.path.push(Seg::Key(key.clone()));
            let put = self.put_or_prune(values, value_col, Input::Tok(value_tok), Some(&key));
            self.path.pop();
            match put.and_then(|kept| {
                if kept {
                    keys.append(&key).map(|_| true)
                } else {
                    Ok(false)
                }
            }) {
                Ok(true) => {
                    if let Some((entry_valid, entry_len)) = entries.as_mut() {
                        entry_valid.append_non_null();
                        **entry_len += 1;
                    }
                    self.strings += entry_strings;
                    kept += 1;
                }
                Ok(false) => {}
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        self.members.truncate(first);
        result?;
        if kept == 0 && len > 0 {
            return Ok(Put::Emptied);
        }
        offsets.push(kept)?;
        valid.append_non_null();
        Ok(Put::Kept)
    }

    /// The members since `first` as `Value`'s map holds them: a repeated key at its first
    /// place with its last value.
    fn dedup_members(&mut self, first: usize) {
        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut names: Vec<Cow<'r, str>> = Vec::new();
        for m in first..self.members.len() {
            let (key, value) = self.members[m];
            let name = self.key_of(key);
            match names.iter().position(|n| *n == name) {
                Some(at) => out[at].1 = value,
                None => {
                    names.push(name);
                    out.push((key, value));
                }
            }
        }
        self.members.truncate(first);
        self.members.extend(out);
    }
}

impl ArrowPlan<'_> {
    /// `rows` normalised and decoded into one struct array, with the values pruned from them
    /// (numbered from `first_row`): the same as normalising each row with
    /// [`NormalisePlan::normalise`] (a row that is not valid JSON being `null`) and decoding
    /// them with `values_to_struct_array`, without building a `Value`. `None` where that
    /// cannot be done exactly (see the module docs), for the `Value` path to take.
    pub fn normalise(
        &self,
        rows: &[Option<&str>],
        first_row: usize,
    ) -> Option<(StructArray, Vec<PrunedValue>)> {
        let mut col = Column::new(&self.root_type).ok()?;
        let mut d = Decoder {
            cfg: self.cfg,
            strict: self.strict,
            misfit: false,
            row: "",
            toks: Vec::new(),
            row_index: 0,
            path: Vec::new(),
            pruned: Vec::new(),
            members: Vec::new(),
            found: Vec::new(),
            generation: 0,
            slots: Vec::new(),
            seen: FxHashSet::default(),
            strings: 0,
        };
        let wrap_root = self.cfg.wrap_root.as_deref();
        for (i, row) in rows.iter().enumerate() {
            let Some(row) = row else {
                col.append_null();
                continue;
            };
            let valid = lex(row, &mut d.toks).is_ok();
            d.row = row;
            d.row_index = first_row + i;
            d.slots.clear();
            if d.found.len() < d.toks.len() {
                d.found.resize(d.toks.len(), (0, 0));
            }
            d.generation = d.generation.wrapping_add(1);
            if d.generation == 0 {
                d.found.fill((0, 0));
                d.generation = 1;
            }
            let input = match (wrap_root, valid) {
                (Some(field), valid) => Input::Wrapped(field, valid.then_some(0)),
                (None, true) => Input::Tok(0),
                (None, false) => Input::Null,
            };
            if self.root.can_prune() && d.prunes(&self.root, input).ok()? {
                d.record_pruned(input);
                col.append_null();
            } else if let Put::Emptied = d.put(&self.root, &mut col, input, None).ok()? {
                col.append_null();
            }
            if d.misfit {
                return None;
            }
        }
        // `arrow-json` holds no more strings in a batch than its tape's `u32` offsets count
        if d.strings >= u64::from(u32::MAX) / 2 {
            return None;
        }
        let array = col.finish().ok()?;
        let array = array.as_any().downcast_ref::<StructArray>()?.clone();
        Some((array, d.pruned))
    }
}
