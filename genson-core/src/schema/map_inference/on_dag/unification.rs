//! `unification.rs` on a DAG: the same functions, case for case, on ids.
//!
//! Each pair is unified as `check_unifiable_schemas` unifies it, the way the borrowed
//! schemas of `unification.rs` are; its owned pairs (`unify_pair` and its kinds) give the
//! same schemas, as `test_owned_unification_matches_the_borrowed` checks. Ids are copied
//! rather than moved, so there is no owned way here.

use super::OnDag;
use crate::dag::{kw, Id, ObjBuf, Sym};
use rustc_hash::FxHashMap;

/// The scalar types, as in `unification.rs`.
const SCALAR_TYPES: [Sym; 4] = [kw::STRING, kw::NUMBER, kw::INTEGER, kw::BOOLEAN];

impl OnDag<'_> {
    fn is_null(&self, id: Id) -> bool {
        self.dag.is_sym(id, kw::NULL)
    }

    /// `normalise_nullable`
    pub(super) fn normalise_nullable(&self, mut schema: Id) -> Id {
        while let Some(&[a, b]) = self.dag.as_array(schema) {
            if self.is_null(a) {
                schema = b;
            } else if self.is_null(b) {
                schema = a;
            } else {
                break;
            }
        }
        schema
    }

    /// `nullable_type`
    fn nullable_type(&self, schema: Id) -> Option<Id> {
        let types = self.dag.as_array(self.dag.get(schema, kw::TYPE)?)?;
        if types.len() == 2 && types.iter().any(|&t| self.is_null(t)) {
            types.iter().copied().find(|&t| !self.is_null(t))
        } else {
            None
        }
    }

    /// `base_type`
    fn base_type(&self, schema: Id) -> Option<Sym> {
        let t = self.dag.get(schema, kw::TYPE)?;
        match self.dag.as_sym(t) {
            Some(t) => Some(t),
            None => self.dag.as_sym(self.nullable_type(schema)?),
        }
    }

    /// `get_scalar_type_name`
    fn scalar_type_name(&self, schema: Id) -> Option<Sym> {
        self.base_type(schema).filter(|t| SCALAR_TYPES.contains(t))
    }

    /// `is_scalar_schema`
    pub(super) fn is_scalar(&self, schema: Id) -> bool {
        self.scalar_type_name(self.normalise_nullable(schema))
            .is_some()
    }

    /// `is_array_schema`
    fn is_array(&self, schema: Id) -> bool {
        self.base_type(self.normalise_nullable(schema)) == Some(kw::ARRAY)
    }

    /// `is_object_schema`
    pub(super) fn is_record(&self, schema: Id) -> bool {
        self.base_type(schema) == Some(kw::OBJECT) && self.dag.get(schema, kw::PROPERTIES).is_some()
    }

    /// `is_map_schema`
    fn is_map(&self, schema: Id) -> bool {
        self.base_type(schema) == Some(kw::OBJECT)
            && self.dag.get(schema, kw::ADDITIONAL_PROPERTIES).is_some()
    }

    /// `is_empty_record_schema`
    fn is_empty_record(&self, schema: Id) -> bool {
        if self.base_type(schema) != Some(kw::OBJECT) {
            return false;
        }
        let values = self.dag.get(schema, kw::ADDITIONAL_PROPERTIES);
        if values.is_some_and(|v| {
            self.dag.as_object(v).is_some()
                || matches!(self.dag.term(v), crate::dag::Term::Bool(true))
        }) {
            return false;
        }
        match self.dag.get(schema, kw::PROPERTIES) {
            None => true,
            Some(properties) => self.dag.as_object(properties).is_some_and(|p| p.is_empty()),
        }
    }

    /// A schema with its `type` replaced (in place) by `t`.
    fn with_type(&mut self, schema: Id, t: Id) -> Id {
        let Some(obj) = self.dag.as_object(schema) else {
            return schema;
        };
        let mut obj = ObjBuf::of(obj);
        obj.insert(kw::TYPE, t);
        obj.build(&mut self.dag)
    }

    /// `["null", t]`
    fn null_and(&mut self, t: Id) -> Id {
        let null = self.dag.string(kw::NULL);
        self.dag.array(&[null, t])
    }

    /// `NonNullView::to_value` of `non_null_view`: two schemas' views are equal when these
    /// are the same, as `Value`s.
    pub(super) fn non_null(&mut self, schema: Id) -> Id {
        if let Some(&known) = self.non_null.get(&schema) {
            return known;
        }
        let view = match self.dag.as_array(schema) {
            Some(pair) if pair.len() == 2 && pair.iter().any(|&v| self.is_null(v)) => {
                pair.iter().copied().find(|&v| !self.is_null(v))
            }
            _ => None,
        };
        let view = match view {
            Some(inner) => inner,
            None => match self.nullable_type(schema) {
                Some(t) => self.with_type(schema, t),
                None => schema,
            },
        };
        self.non_null.insert(schema, view);
        view
    }

    /// `try_make_nullable_union`
    fn try_make_nullable_union(&mut self, a: Id, b: Id) -> Option<Id> {
        if !self.is_null(self.dag.get(a, kw::TYPE)?) {
            return None;
        }
        let other = self.dag.get(b, kw::TYPE).filter(|&t| !self.is_null(t))?;
        self.dag.as_object(b)?;
        let types = self.null_and(other);
        Some(self.with_type(b, types))
    }

    /// `schemas_compatible_unequal`
    fn schemas_compatible_unequal(&mut self, existing: Id, new: Id) -> Option<Id> {
        if let Some(result) = self
            .try_make_nullable_union(existing, new)
            .or_else(|| self.try_make_nullable_union(new, existing))
        {
            return Some(result);
        }
        let (existing_type, new_type) = (self.nullable_type(existing), self.nullable_type(new));
        existing_type.or(new_type)?;
        let existing_view = match existing_type {
            Some(t) => self.with_type(existing, t),
            None => existing,
        };
        let new_view = match new_type {
            Some(t) => self.with_type(new, t),
            None => new,
        };
        if !self.dag.same(existing_view, new_view) {
            return None;
        }
        match self.dag.get(existing_view, kw::TYPE) {
            Some(inner) => {
                let types = self.null_and(inner);
                Some(self.with_type(existing_view, types))
            }
            None => Some(existing_view),
        }
    }

    /// `path.field`, as `format!("{path}.{field}")`.
    pub(super) fn join(&mut self, path: Sym, field: Sym) -> Sym {
        if let Some(&joined) = self.paths.get(&(path, field)) {
            return joined;
        }
        let joined = format!("{}.{}", self.dag.name(path), self.dag.name(field));
        let joined = self.dag.sym(&joined);
        self.paths.insert((path, field), joined);
        joined
    }

    /// `make_promoted_scalar_key`
    pub(super) fn promoted_key(&mut self, field: Sym, scalar_type: Sym) -> Sym {
        let key = crate::schema::make_promoted_scalar_key(
            self.dag.name(field),
            self.dag.name(scalar_type),
        );
        self.dag.sym(&key)
    }

    /// `{"type": "object", "properties": {<key>: schema}}`
    pub(super) fn record_of(&mut self, key: Sym, schema: Id) -> Id {
        let object = self.dag.string(kw::OBJECT);
        let properties = self.dag.object(&[(key, schema)]);
        self.dag
            .object(&[(kw::TYPE, object), (kw::PROPERTIES, properties)])
    }

    /// `{"type": t}`
    pub(super) fn typed(&mut self, t: Sym) -> Id {
        let t = self.dag.string(t);
        self.dag.object(&[(kw::TYPE, t)])
    }

    /// `promotion_field_name`
    fn promotion_field_name(&mut self, path: Sym) -> Sym {
        let name = self
            .dag
            .name(path)
            .rsplit('.')
            .find(|s| !s.is_empty() && *s != "items" && *s != "additionalProperties")
            .unwrap_or("value")
            .to_string();
        self.dag.sym(&name)
    }

    /// `try_scalar_promotion`
    fn try_scalar_promotion(
        &mut self,
        object_schema: Id,
        scalar_schema: Id,
        field: Sym,
        path: Sym,
    ) -> Option<Id> {
        let scalar_type = self.scalar_type_name(scalar_schema)?;
        let key = self.promoted_key(field, scalar_type);
        let promoted = self.record_of(key, scalar_schema);
        let field_path = self.join(path, field);
        let result = self.check_unifiable_schemas(&[object_schema, promoted], field_path)?;
        Some(match self.dag.as_object(result) {
            Some(obj) => {
                let mut obj = ObjBuf::of(obj);
                obj.shift_remove(kw::REQUIRED);
                obj.build(&mut self.dag)
            }
            None => result,
        })
    }

    /// `extract_field_from_nullable_schema`
    fn extract(&self, schema: Id, keyword: Sym) -> Option<Id> {
        self.dag.get(self.normalise_nullable(schema), keyword)
    }

    /// `unify_array_schemas`
    fn unify_array_schemas(&mut self, schemas: &[Id], path: Sym) -> Option<Id> {
        if schemas.is_empty() {
            return None;
        }
        let items: Vec<Id> = schemas
            .iter()
            .filter_map(|&schema| self.extract(schema, kw::ITEMS))
            .filter(|&items| self.dag.as_object(items).is_none_or(|o| !o.is_empty()))
            .collect();
        let array = self.dag.string(kw::ARRAY);
        let items = match items.as_slice() {
            [] => return Some(self.dag.object(&[(kw::TYPE, array)])),
            [only] => *only,
            _ => {
                let items_path = self.join(path, kw::ITEMS);
                self.check_unifiable_item_schemas(&items, items_path)?
            }
        };
        Some(self.dag.object(&[(kw::TYPE, array), (kw::ITEMS, items)]))
    }

    /// `unify_scalar_schemas`
    fn unify_scalar_schemas(&mut self, schemas: &[Id]) -> Option<Id> {
        let mut base_types = schemas.iter().filter_map(|&schema| self.base_type(schema));
        let first = base_types.next()?;
        if base_types.any(|t| t != first) {
            return None;
        }
        let first = self.dag.string(first);
        let types = self.null_and(first);
        Some(self.dag.object(&[(kw::TYPE, types)]))
    }

    /// `unify_map_schemas`
    fn unify_map_schemas(&mut self, schemas: &[Id], path: Sym) -> Option<Id> {
        let values: Vec<Id> = schemas
            .iter()
            .map(|&schema| self.extract(schema, kw::ADDITIONAL_PROPERTIES))
            .collect::<Option<_>>()?;
        let null = self.typed(kw::NULL);
        let (mut nulls, mut typed) = (false, Vec::new());
        for v in values {
            match self.dag.same(v, null) {
                true => nulls = true,
                false => typed.push(v),
            }
        }
        let values_path = self.join(path, kw::ADDITIONAL_PROPERTIES);
        let values = match typed.as_slice() {
            [] => null,
            _ => {
                let values = self.check_unifiable_schemas(&typed, values_path)?;
                match nulls {
                    true => self.make_nullable(values),
                    false => values,
                }
            }
        };
        let object = self.dag.string(kw::OBJECT);
        Some(
            self.dag
                .object(&[(kw::TYPE, object), (kw::ADDITIONAL_PROPERTIES, values)]),
        )
    }

    /// `unify_field_schemas_sequential`
    fn unify_field_schemas_sequential(
        &mut self,
        field: Sym,
        schemas: &[Id],
        path: Sym,
    ) -> Option<Id> {
        let (&first, rest) = schemas.split_first()?;
        let field_path = self.join(path, field);
        let mut unified = first;
        for &new in rest {
            if self.dag.same(unified, new) {
                continue;
            }
            unified = self.unify_field_pair(unified, new, field, field_path, path)?;
        }
        Some(unified)
    }

    /// `unify_maps_beside_null_values`
    fn unify_maps_beside_null_values(&mut self, a: Id, b: Id, path: Sym) -> Option<Id> {
        if !(self.is_map(a) && self.is_map(b)) {
            return None;
        }
        let a_values = self.extract(a, kw::ADDITIONAL_PROPERTIES)?;
        let b_values = self.extract(b, kw::ADDITIONAL_PROPERTIES)?;
        let null = self.typed(kw::NULL);
        let (a_view, b_view) = (self.non_null(a_values), self.non_null(b_values));
        if !(self.dag.same(a_values, null)
            || self.dag.same(b_values, null)
            || self.dag.same(a_view, b_view))
        {
            return None;
        }
        let map = self.unify_map_schemas(&[a, b], path)?;
        Some(match self.nullable_type(a).or(self.nullable_type(b)) {
            Some(_) => self.make_nullable(map),
            None => map,
        })
    }

    /// `unify_field_pair`
    fn unify_field_pair(
        &mut self,
        unified: Id,
        new: Id,
        field: Sym,
        field_path: Sym,
        path: Sym,
    ) -> Option<Id> {
        if let Some(compatible) = self.schemas_compatible_unequal(unified, new) {
            return Some(compatible);
        }
        if let Some(map) = self.unify_maps_beside_null_values(unified, new, field_path) {
            return Some(map);
        }
        let record_like = |on: &Self, s: Id| on.is_record(s) || on.is_empty_record(s);
        if (self.is_array(unified) && self.is_array(new))
            || (record_like(self, unified) && record_like(self, new))
        {
            return self.unify_pair(unified, new, field_path);
        }
        if !self.config.wrap_scalars {
            return None;
        }
        if self.is_record(unified) && self.is_scalar(new) {
            self.try_scalar_promotion(unified, new, field, path)
        } else if self.is_record(new) && self.is_scalar(unified) {
            self.try_scalar_promotion(new, unified, field, path)
        } else if self.is_scalar(unified) && self.is_scalar(new) {
            self.try_mixed_scalar_promotion(unified, new, field)
        } else {
            // An array beside a record or scalar: one array holding them, as for a union
            self.unify_anyof_schemas(&[unified, new], field)
        }
    }

    /// `check_unifiable_schemas` of a pair, remembered: unifying is a function of the two
    /// schemas and the path, and the folds below meet the same pairs again and again.
    fn unify_pair(&mut self, a: Id, b: Id, path: Sym) -> Option<Id> {
        if let Some(&known) = self.pairs.get(&(a, b, path)) {
            return known;
        }
        let unified = self.check_unifiable_schemas(&[a, b], path);
        self.pairs.insert((a, b, path), unified);
        unified
    }

    /// `unify_field_schemas_parallel`, here in turn: the same halves, the same result.
    fn unify_field_schemas_halves(&mut self, field: Sym, schemas: &[Id], path: Sym) -> Option<Id> {
        if schemas.len() < 10 {
            return self.unify_field_schemas_sequential(field, schemas, path);
        }
        let (left, right) = schemas.split_at(schemas.len() / 2);
        let left = self.unify_field_schemas_halves(field, left, path);
        let right = self.unify_field_schemas_halves(field, right, path);
        let field_path = self.join(path, field);
        self.unify_pair(left?, right?, field_path)
    }

    /// `unify_field_schemas`
    fn unify_field_schemas(&mut self, field: Sym, schemas: &[Id], path: Sym) -> Option<Id> {
        let needs_scalar_promo = self.config.wrap_scalars
            && ((schemas.iter().any(|&s| self.is_scalar(s))
                && schemas.iter().any(|&s| self.is_record(s)))
                || (schemas.iter().any(|&s| self.is_array(s))
                    && schemas.iter().any(|&s| !self.is_array(s))));
        if needs_scalar_promo || schemas.len() < 50 {
            self.unify_field_schemas_sequential(field, schemas, path)
        } else {
            self.unify_field_schemas_halves(field, schemas, path)
        }
    }

    /// `unify_record_schemas`
    fn unify_record_schemas(&mut self, schemas: &[Id], path: Sym) -> Option<Id> {
        // Each field's schemas in order of first appearance, and where each field is
        let mut fields: Vec<(Sym, Vec<Id>)> = Vec::new();
        let mut at: FxHashMap<Sym, usize> = FxHashMap::default();
        let mut counts: FxHashMap<Sym, usize> = FxHashMap::default();
        let mut required_in: FxHashMap<Sym, usize> = FxHashMap::default();
        let mut unified_anyofs: Vec<(Sym, Id)> = Vec::new();
        let push =
            |fields: &mut Vec<(Sym, Vec<Id>)>, at: &mut FxHashMap<Sym, usize>, name, schema| {
                let i = *at.entry(name).or_insert_with(|| {
                    fields.push((name, Vec::new()));
                    fields.len() - 1
                });
                fields[i].1.push(schema);
            };
        for &schema in schemas {
            if let Some(required) = self.extract(schema, kw::REQUIRED) {
                if let Some(names) = self.dag.as_array(required) {
                    for name in names.iter().filter_map(|&n| self.dag.as_sym(n)) {
                        *required_in.entry(name).or_default() += 1;
                    }
                }
            }
            let properties = match self.extract(schema, kw::PROPERTIES) {
                Some(properties) if self.dag.as_object(properties).is_some() => {
                    self.dag.as_object(properties).unwrap().to_vec()
                }
                _ if self.is_empty_record(schema) => Vec::new(),
                _ => return None,
            };
            for (name, field_schema) in properties {
                *counts.entry(name).or_default() += 1;
                let field_schema = self.normalise_nullable(field_schema);
                if let Some(any_of) = self.dag.get(field_schema, kw::ANY_OF) {
                    if let Some(any_of) = self.dag.as_array(any_of).map(<[Id]>::to_vec) {
                        if let Some(unified) = self.unify_anyof_schemas(&any_of, name) {
                            unified_anyofs.push((name, unified));
                            continue;
                        }
                    }
                }
                push(&mut fields, &mut at, name, field_schema);
            }
        }
        for (name, unified) in unified_anyofs {
            push(&mut fields, &mut at, name, unified);
        }

        let merge_start = std::time::Instant::now();
        let mut unified = Vec::with_capacity(fields.len());
        for (name, field_schemas) in &fields {
            unified.push(self.unify_field_schemas(*name, field_schemas, path)?);
        }
        if self.config.profile && schemas.len() > 50 {
            anstream::eprintln!("  Merge loop took {:?}", merge_start.elapsed());
        }

        // Fields in every record first, then the rest; a field every record requires is kept
        // as it is and required, any other made nullable
        let n = schemas.len();
        let required_by_all = |name: Sym| required_in.get(&name) == Some(&n);
        let (in_all, in_some): (Vec<_>, Vec<_>) = fields
            .iter()
            .zip(unified)
            .map(|((name, _), field_schema)| (*name, field_schema))
            .partition(|(name, _)| counts[name] == n);
        let mut required = Vec::new();
        let mut properties = Vec::with_capacity(in_all.len() + in_some.len());
        for (name, field_schema) in in_all {
            if required_by_all(name) {
                required.push(name);
                properties.push((name, field_schema));
            } else {
                properties.push((name, self.make_nullable(field_schema)));
            }
        }
        for (name, field_schema) in in_some {
            properties.push((name, self.make_nullable(field_schema)));
        }
        let object = self.dag.string(kw::OBJECT);
        let properties = self.dag.object(&properties);
        let mut result = vec![(kw::TYPE, object), (kw::PROPERTIES, properties)];
        if !required.is_empty() {
            let names: Vec<Id> = required
                .into_iter()
                .map(|name| self.dag.string(name))
                .collect();
            result.push((kw::REQUIRED, self.dag.array(&names)));
        }
        Some(self.dag.object(&result))
    }

    /// `make_nullable`
    pub(super) fn make_nullable(&mut self, schema: Id) -> Id {
        match self.dag.get(schema, kw::TYPE) {
            Some(t) if self.dag.as_sym(t).is_some_and(|t| t != kw::NULL) => {
                let types = self.null_and(t);
                self.with_type(schema, types)
            }
            Some(t) if self.dag.as_sym(t).is_some() || self.dag.as_array(t).is_some() => schema,
            _ => {
                let null = self.typed(kw::NULL);
                let any_of = self.dag.array(&[null, schema]);
                self.dag.object(&[(kw::ANY_OF, any_of)])
            }
        }
    }

    /// `try_mixed_scalar_promotion`
    fn try_mixed_scalar_promotion(&mut self, existing: Id, new: Id, field: Sym) -> Option<Id> {
        let existing_type = self.scalar_type_name(existing)?;
        let new_type = self.scalar_type_name(new)?;
        if existing_type == new_type {
            return None;
        }
        let existing_key = self.promoted_key(field, existing_type);
        let new_key = self.promoted_key(field, new_type);
        let object = self.dag.string(kw::OBJECT);
        let properties = self.dag.object(&[(existing_key, existing), (new_key, new)]);
        Some(
            self.dag
                .object(&[(kw::TYPE, object), (kw::PROPERTIES, properties)]),
        )
    }

    /// `unify_rewritten_into_array`
    pub(super) fn unify_rewritten_into_array(&mut self, branches: &[Id]) -> Option<Id> {
        if !self.config.wrap_scalars {
            return None;
        }
        self.unify_into_array(branches)
    }

    /// `without_null`
    fn without_null(&mut self, schema: Id) -> (Option<Id>, bool) {
        let inner = self.normalise_nullable(schema);
        let wrapped = inner != schema;
        let Some(t) = self.dag.get(inner, kw::TYPE) else {
            return (Some(inner), wrapped);
        };
        if self.is_null(t) {
            return (None, true);
        }
        let Some(types) = self.dag.as_array(t) else {
            return (Some(inner), wrapped);
        };
        if !types.iter().any(|&t| self.is_null(t)) {
            return (Some(inner), wrapped);
        }
        let rest: Vec<Id> = types
            .iter()
            .copied()
            .filter(|&t| !self.is_null(t))
            .collect();
        let t = match rest.as_slice() {
            [] => return (None, true),
            [only] => *only,
            _ => self.dag.array(&rest),
        };
        (Some(self.with_type(inner, t)), true)
    }

    /// `unify_into_array`
    fn unify_into_array(&mut self, schemas: &[Id]) -> Option<Id> {
        let (mut arrays, mut others, mut nullable) = (Vec::new(), Vec::new(), false);
        for &schema in schemas {
            let (schema, null) = self.without_null(schema);
            nullable |= null;
            match schema {
                Some(schema)
                    if self
                        .dag
                        .get(schema, kw::TYPE)
                        .is_none_or(|t| self.dag.as_sym(t).is_none()) =>
                {
                    return None
                }
                Some(schema) if self.is_array(schema) => arrays.push(schema),
                Some(schema) => others.push(schema),
                None => {}
            }
        }
        if arrays.is_empty() || others.is_empty() {
            return None;
        }
        let mut null_items = false;
        let mut items: Vec<Id> = Vec::new();
        for items_of in arrays
            .iter()
            .filter_map(|&schema| self.dag.get(schema, kw::ITEMS))
            .filter(|&items| self.dag.as_object(items).is_none_or(|o| !o.is_empty()))
        {
            if self.is_null_schema(items_of) {
                null_items = true;
            } else {
                items.push(items_of);
            }
        }
        items.extend(others);
        let items = match items.as_slice() {
            [only] => *only,
            _ => match self.check_unifiable_schemas(&items, kw::EMPTY) {
                Some(unified) => unified,
                None => self.unify_anyof_schemas(&items, kw::EMPTY)?,
            },
        };
        let items = if null_items {
            self.make_nullable(items)
        } else {
            items
        };
        let array = self.dag.string(kw::ARRAY);
        let array = self.dag.object(&[(kw::TYPE, array), (kw::ITEMS, items)]);
        if !nullable {
            return Some(array);
        }
        let null = self.typed(kw::NULL);
        let branches = self.dag.array(&[array, null]);
        Some(self.dag.object(&[(kw::ANY_OF, branches)]))
    }

    /// `type_union_clashes`
    pub(super) fn type_union_clashes(&self, schema: Id) -> bool {
        let Some(types) = self
            .dag
            .get(schema, kw::TYPE)
            .and_then(|t| self.dag.as_array(t))
        else {
            return false;
        };
        let kinds: Vec<Sym> = types.iter().filter_map(|&t| self.dag.as_sym(t)).collect();
        self.kinds_clash(kinds)
    }

    /// `kinds_clash`
    fn kinds_clash(&self, kinds: impl IntoIterator<Item = Sym>) -> bool {
        let kinds: std::collections::BTreeSet<Sym> = kinds.into_iter().collect();
        let coerced = self.config.coerce_strings
            && kinds
                .iter()
                .any(|&t| t == kw::INTEGER || t == kw::NUMBER || t == kw::BOOLEAN);
        let kinds: Vec<Sym> = kinds
            .into_iter()
            .filter(|&t| t != kw::NULL && !(coerced && t == kw::STRING))
            .collect();
        kinds.len() >= 2 && !kinds.iter().all(|&t| t == kw::INTEGER || t == kw::NUMBER)
    }

    /// `type_union_branches`
    pub(super) fn type_union_branches(&mut self, schema: Id) -> Option<Vec<Id>> {
        let types = self.dag.as_array(self.dag.get(schema, kw::TYPE)?)?;
        let mut types: Vec<Sym> = types
            .iter()
            .map(|&t| self.dag.as_sym(t))
            .collect::<Option<_>>()?;
        if types.iter().filter(|&&t| t != kw::NULL).count() < 2 {
            return None;
        }
        types.sort_by_key(|&t| crate::schema::type_string_rank(self.dag.name(t)));
        let branches = types
            .into_iter()
            .map(|t| {
                let keys: &[Sym] = if t == kw::OBJECT {
                    &[kw::PROPERTIES, kw::REQUIRED, kw::ADDITIONAL_PROPERTIES]
                } else if t == kw::ARRAY {
                    &[kw::ITEMS]
                } else {
                    &[]
                };
                let mut fields = vec![(kw::TYPE, self.dag.string(t))];
                fields.extend(
                    keys.iter()
                        .filter_map(|&key| Some((key, self.dag.get(schema, key)?))),
                );
                self.dag.object(&fields)
            })
            .collect();
        Some(branches)
    }

    /// `is_null_schema`
    pub(super) fn is_null_schema(&self, schema: Id) -> bool {
        self.dag
            .get(schema, kw::TYPE)
            .is_some_and(|t| self.is_null(t))
    }

    /// `scalar_kinds_clash`
    fn scalar_kinds_clash(&self, schemas: &[Id]) -> bool {
        let kinds: Vec<Sym> = schemas
            .iter()
            .filter_map(|&s| self.scalar_type_name(self.normalise_nullable(s)))
            .collect();
        self.kinds_clash(kinds)
    }

    /// `unify_anyof_schemas`
    pub(super) fn unify_anyof_schemas(&mut self, schemas: &[Id], field: Sym) -> Option<Id> {
        if !self.config.wrap_scalars {
            return None;
        }
        let is_union =
            |on: &Self, s: Id| on.dag.get(s, kw::ANY_OF).is_some() || on.type_union_clashes(s);
        let split: Option<Vec<Id>> = if schemas.iter().any(|&s| is_union(self, s)) {
            let mut split = Vec::new();
            for &s in schemas {
                self.union_branches(s, &mut split);
            }
            Some(split)
        } else {
            None
        };
        let schemas = split.as_deref().unwrap_or(schemas);
        if let Some(array) = self.unify_into_array(schemas) {
            return Some(array);
        }
        let nullable = schemas.iter().any(|&s| self.is_null_schema(s));
        let non_null: Vec<Id> = schemas
            .iter()
            .copied()
            .filter(|&s| !self.is_null_schema(s))
            .collect();
        if nullable && non_null.len() >= 2 {
            if let Some(unified) = self.check_unifiable_schemas(&non_null, field) {
                return Some(self.make_nullable(unified));
            }
        }
        let number = |on: &Self, s: Id| matches!(on.base_type(s), Some(kw::INTEGER | kw::NUMBER));
        if non_null.len() >= 2 && non_null.iter().all(|&s| number(self, s)) {
            let typed_null = |on: &Self, s: Id| {
                on.dag
                    .get(s, kw::TYPE)
                    .and_then(|t| on.dag.as_array(t))
                    .is_some_and(|t| t.iter().any(|&t| on.is_null(t)))
            };
            let widened = self.typed(kw::NUMBER);
            return Some(
                if nullable || non_null.iter().any(|&s| typed_null(self, s)) {
                    self.make_nullable(widened)
                } else {
                    widened
                },
            );
        }
        let has_scalars = schemas.iter().any(|&s| self.is_scalar(s));
        let has_objects = schemas
            .iter()
            .any(|&s| self.is_record(s) || self.is_empty_record(s));
        if !has_scalars || !(has_objects || self.scalar_kinds_clash(schemas)) {
            return None;
        }
        let branches: Vec<Id> = schemas
            .iter()
            .copied()
            .filter(|&schema| !self.is_null_schema(schema))
            .collect();
        let promoted: Vec<Id> = branches
            .iter()
            .map(|&schema| {
                if !self.is_scalar(schema) {
                    return Some(schema);
                }
                let scalar_type = self.scalar_type_name(schema)?;
                let key = self.promoted_key(field, scalar_type);
                Some(self.record_of(key, schema))
            })
            .collect::<Option<_>>()?;
        let unified = self.check_unifiable_schemas(&promoted, field)?;
        Some(if nullable {
            self.make_nullable(unified)
        } else {
            unified
        })
    }

    /// `union_branches`
    fn union_branches(&mut self, schema: Id, out: &mut Vec<Id>) {
        if let Some(any_of) = self
            .dag
            .get(schema, kw::ANY_OF)
            .and_then(|a| self.dag.as_array(a))
            .map(<[Id]>::to_vec)
        {
            for branch in any_of {
                self.union_branches(branch, out);
            }
            return;
        }
        let branches = self
            .type_union_clashes(schema)
            .then(|| self.type_union_branches(schema))
            .flatten();
        match branches {
            Some(branches) => out.extend(branches),
            None => out.push(schema),
        }
    }

    /// `check_unifiable_schemas`
    pub(super) fn check_unifiable_schemas(&mut self, schemas: &[Id], path: Sym) -> Option<Id> {
        if schemas.is_empty() {
            return None;
        }
        if schemas.iter().all(|&s| self.is_array(s)) {
            return self.unify_array_schemas(schemas, path);
        }
        if schemas
            .iter()
            .all(|&s| self.is_map(s) || self.is_empty_record(s))
        {
            let maps: Vec<Id> = schemas
                .iter()
                .copied()
                .filter(|&s| self.is_map(s))
                .collect();
            if maps.is_empty() {
                let object = self.dag.string(kw::OBJECT);
                let string = self.typed(kw::STRING);
                return Some(
                    self.dag
                        .object(&[(kw::TYPE, object), (kw::ADDITIONAL_PROPERTIES, string)]),
                );
            }
            return self.unify_map_schemas(&maps, path);
        }
        if schemas
            .iter()
            .all(|&s| self.is_record(s) || self.is_empty_record(s))
        {
            return self.unify_record_schemas(schemas, path);
        }
        if schemas.iter().all(|&s| self.is_scalar(s)) {
            return self.unify_scalar_schemas(schemas);
        }
        None
    }

    /// `check_unifiable_item_schemas`
    pub(super) fn check_unifiable_item_schemas(&mut self, schemas: &[Id], path: Sym) -> Option<Id> {
        if let Some(unified) = self.check_unifiable_schemas(schemas, path) {
            return Some(unified);
        }
        let records_and_scalars = schemas
            .iter()
            .all(|&s| self.is_scalar(s) || self.is_record(s) || self.is_empty_record(s));
        let arrays_and_others =
            schemas.iter().any(|&s| self.is_array(s)) && schemas.iter().any(|&s| !self.is_array(s));
        if !(self.config.wrap_scalars && (records_and_scalars || arrays_and_others)) {
            return None;
        }
        let field = self.promotion_field_name(path);
        self.unify_anyof_schemas(schemas, field)
    }
}
