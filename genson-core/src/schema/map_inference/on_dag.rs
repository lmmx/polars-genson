//! The rewrite of `map_inference.rs` on a DAG of the merged schema, so that the schema is
//! never written out whole: the builder writes it as a DAG (`SchemaBuilder::to_dag`), in
//! which each distinct part is stored once, and only the rewritten schema becomes a `Value`.
//!
//! Each function here is the one of the same name in `map_inference.rs` (or
//! `unification.rs`, in `on_dag/unification.rs`), case for case, with ids for `Value`s:
//! - a schema is rewritten or unified once for each distinct schema it is, in each field
//!   and path it is met in, as each is a function of these (and the config);
//! - `Value` equality, which ignores the order of an object's keys, is `Dag::same`;
//! - what `map_inference.rs` does on the rayon pool is done here in turn, to the same result.
//!
//! Debugging takes the `Value` way, so that it reports as it did.

use crate::dag::{kw, Dag, Id, Obj, ObjBuf, Sym, Term};
use crate::schema::core::SchemaInferenceConfig;
use rustc_hash::FxHashMap;

mod unification;

/// No field name: the root, a list's items or a map's values.
const NO_FIELD: Sym = Sym::MAX;

/// A DAG of the merged schema and what its rewrite has found so far.
pub(crate) struct OnDag<'c> {
    pub(crate) dag: Dag,
    config: &'c SchemaInferenceConfig,
    /// `non_null` of each schema asked for
    non_null: FxHashMap<Id, Id>,
    /// `contains_anyof` of each schema asked for
    any_of: FxHashMap<Id, bool>,
    /// `rewrite_objects` of each schema, in each field and position met
    rewritten: FxHashMap<(Id, Sym, bool), Id>,
    /// `check_unifiable_schemas` of each pair unified, at each path
    pairs: FxHashMap<(Id, Id, Sym), Option<Id>>,
    /// Each path joined with a field
    paths: FxHashMap<(Sym, Sym), Sym>,
}

impl<'c> OnDag<'c> {
    pub(crate) fn new(dag: Dag, config: &'c SchemaInferenceConfig) -> Self {
        OnDag {
            dag,
            config,
            non_null: FxHashMap::default(),
            any_of: FxHashMap::default(),
            rewritten: FxHashMap::default(),
            pairs: FxHashMap::default(),
            paths: FxHashMap::default(),
        }
    }

    fn field_name(&self, field: Option<Sym>) -> Option<&str> {
        field.map(|f| self.dag.name(f))
    }

    fn is_forced_scalar(&self, name: &str) -> bool {
        self.config.force_scalar_promotion.contains(name)
    }

    fn in_no_unify(&self, props: Obj<'_>) -> bool {
        props
            .keys()
            .any(|k| self.config.no_unify.contains(self.dag.name(k)))
    }

    /// `contains_anyof`
    fn contains_anyof(&mut self, value: Id) -> bool {
        if let Some(&known) = self.any_of.get(&value) {
            return known;
        }
        let contains = match self.dag.term(value) {
            Term::Object(obj) => {
                obj.keys().any(|k| k == kw::ANY_OF) || {
                    let values: Vec<Id> = obj.values().collect();
                    values.into_iter().any(|v| self.contains_anyof(v))
                }
            }
            Term::Array(items) => {
                let items = items.to_vec();
                items.into_iter().any(|v| self.contains_anyof(v))
            }
            _ => false,
        };
        self.any_of.insert(value, contains);
        contains
    }

    /// `forced_map_value_schema`
    pub(crate) fn forced_map_value_schema(&mut self, obj: &ObjBuf, field: Option<Sym>) -> Id {
        if let Some(values) = obj
            .get(kw::ADDITIONAL_PROPERTIES)
            .filter(|&v| self.dag.as_object(v).is_some())
        {
            return values;
        }
        let Some(props) = obj
            .get(kw::PROPERTIES)
            .and_then(|p| self.dag.as_object(p))
            .filter(|p| !p.is_empty())
        else {
            return self.typed(kw::NULL);
        };
        let values: Vec<Id> = props.values().collect();
        let no_unify = self.in_no_unify(props);
        let first = self.non_null(values[0]);
        if values.iter().all(|&v| {
            let view = self.non_null(v);
            self.dag.same(view, first)
        }) {
            return first;
        }
        if no_unify {
            return self.typed(kw::STRING);
        }
        let path = field.unwrap_or(kw::EMPTY);
        match self.check_unifiable_schemas(&values, path) {
            Some(unified) => unified,
            None => self.typed(kw::STRING),
        }
    }

    /// `process_anyof_unions`
    fn process_anyof_unions(&mut self, schema: Id, field: Sym) -> Id {
        match self.dag.term(schema) {
            Term::Object(obj) => {
                let mut obj = ObjBuf::of(obj);
                if let Some(any_of) = obj
                    .get(kw::ANY_OF)
                    .and_then(|a| self.dag.as_array(a))
                    .map(<[Id]>::to_vec)
                {
                    if let Some(unified) = self.unify_anyof_schemas(&any_of, field) {
                        return self.process_anyof_unions(unified, field);
                    }
                }
                if let Some(props) = obj.get(kw::PROPERTIES) {
                    if let Some(entries) = self.dag.as_object(props).map(Obj::to_vec) {
                        let entries: Vec<(Sym, Id)> = entries
                            .into_iter()
                            .map(|(k, v)| (k, self.process_anyof_unions(v, k)))
                            .collect();
                        let props = self.dag.object(&entries);
                        obj.insert(kw::PROPERTIES, props);
                    }
                } else {
                    for entry in &mut obj.0 {
                        entry.1 = self.process_anyof_unions(entry.1, field);
                    }
                }
                obj.build(&mut self.dag)
            }
            Term::Array(items) => {
                let items: Vec<Id> = items.to_vec();
                let items: Vec<Id> = items
                    .into_iter()
                    .map(|v| self.process_anyof_unions(v, field))
                    .collect();
                self.dag.array(&items)
            }
            _ => schema,
        }
    }

    /// `is_object_type`
    fn is_object_type(&self, t: Option<Id>) -> bool {
        let Some(t) = t else { return false };
        match self.dag.term(t) {
            Term::String(s) => s == kw::OBJECT,
            Term::Array(ts) => {
                ts.len() == 2
                    && ts.iter().any(|&t| self.dag.is_sym(t, kw::NULL))
                    && ts.iter().any(|&t| self.dag.is_sym(t, kw::OBJECT))
            }
            _ => false,
        }
    }

    /// `check_force_parent_field_types`, of an object's `properties`
    fn check_force_parent_field_types(&self, props: Obj<'_>) -> Option<&'c str> {
        let config = self.config;
        for (prop_key, forced_parent_type) in &config.force_parent_field_types {
            if props.keys().any(|k| self.dag.name(k) == prop_key) {
                return Some(forced_parent_type.as_str());
            }
        }
        None
    }

    /// `rewrite_objects`, returning the rewritten schema.
    pub(crate) fn rewrite_objects(&mut self, schema: Id, field: Option<Sym>, is_root: bool) -> Id {
        let key = (schema, field.unwrap_or(NO_FIELD), is_root);
        if let Some(&known) = self.rewritten.get(&key) {
            return known;
        }
        let rewritten = self.rewrite_objects_anew(schema, field, is_root);
        self.rewritten.insert(key, rewritten);
        rewritten
    }

    fn rewrite_objects_anew(&mut self, schema: Id, field: Option<Sym>, is_root: bool) -> Id {
        let config = self.config;
        if let Some(promoted) = self.promote_forced_scalar(schema, field) {
            return promoted;
        }
        let mut obj = match self.dag.term(schema) {
            Term::Object(obj) => ObjBuf::of(obj),
            Term::Array(items) => {
                let items: Vec<Id> = items.to_vec();
                let items: Vec<Id> = items
                    .into_iter()
                    .map(|item| self.rewrite_objects(item, None, false))
                    .collect();
                return self.dag.array(&items);
            }
            _ => return schema,
        };

        // --- Forced overrides by field name ---
        let forced = self
            .field_name(field)
            .and_then(|name| config.force_field_types.get(name));
        if let Some(forced) = forced {
            match forced.as_str() {
                "map" => {
                    let values = self.forced_map_value_schema(&obj, field);
                    let values = self.rewrite_objects(values, None, false);
                    obj.shift_remove(kw::PROPERTIES);
                    obj.shift_remove(kw::REQUIRED);
                    obj.insert(kw::ADDITIONAL_PROPERTIES, values);
                    return obj.build(&mut self.dag);
                }
                "record" => {
                    self.rewrite_fields(&mut obj);
                    return obj.build(&mut self.dag);
                }
                _ => {}
            }
        }

        // --- anyOf unions: scalars among records are promoted (when `wrap_scalars` is set) ---
        if let Some(any_of) = obj
            .get(kw::ANY_OF)
            .and_then(|a| self.dag.as_array(a))
            .map(<[Id]>::to_vec)
        {
            if let Some(unified) = self.unify_anyof_schemas(&any_of, field.unwrap_or(kw::EMPTY)) {
                return self.rewrite_objects(unified, field, is_root);
            }
            let branches: Vec<Id> = any_of
                .into_iter()
                .map(|branch| self.rewrite_objects(branch, field, false))
                .collect();
            let branches = self.dag.array(&branches);
            obj.insert(kw::ANY_OF, branches);
        }

        // --- An object empty in every row, where every object is a map candidate ---
        if config.map_threshold == 0
            && !(is_root && config.no_root_map)
            && self.is_object_type(obj.get(kw::TYPE))
            && obj.get(kw::ADDITIONAL_PROPERTIES).is_none()
            && obj
                .get(kw::PROPERTIES)
                .and_then(|p| self.dag.as_object(p))
                .is_none_or(|p| p.is_empty())
        {
            obj.shift_remove(kw::PROPERTIES);
            obj.shift_remove(kw::REQUIRED);
            let null = self.typed(kw::NULL);
            obj.insert(kw::ADDITIONAL_PROPERTIES, null);
            return obj.build(&mut self.dag);
        }

        // --- Heuristic rewrite ---
        if let Some(props_id) = obj
            .get(kw::PROPERTIES)
            .filter(|&p| self.dag.as_object(p).is_some())
        {
            let props = self.dag.as_object(props_id).unwrap();
            if self.check_force_parent_field_types(props) == Some("record") {
                self.rewrite_fields(&mut obj);
                return obj.build(&mut self.dag);
            }
            let holds_promoted = props
                .keys()
                .any(|k| self.is_forced_scalar(self.dag.name(k)));
            if self
                .field_name(field)
                .is_some_and(|f| self.is_forced_scalar(f))
                || holds_promoted
            {
                self.rewrite_fields(&mut obj);
                return obj.build(&mut self.dag);
            }
            if let Some(values) = obj.get(kw::ADDITIONAL_PROPERTIES) {
                if props.is_empty() {
                    let values = self.rewrite_objects(values, None, false);
                    obj.insert(kw::ADDITIONAL_PROPERTIES, values);
                    return obj.build(&mut self.dag);
                }
            }

            let may_be_map =
                props.len() >= config.map_threshold && !(is_root && config.no_root_map);
            if let Some(values) = may_be_map
                .then(|| self.identical_record_values(props_id))
                .flatten()
            {
                obj.shift_remove(kw::PROPERTIES);
                obj.shift_remove(kw::REQUIRED);
                let values = self.rewrite_objects(values, None, false);
                obj.insert(kw::ADDITIONAL_PROPERTIES, values);
                return obj.build(&mut self.dag);
            }
            let required_key_count = obj
                .get(kw::REQUIRED)
                .and_then(|r| self.dag.as_array(r))
                .map_or(0, |r| r.len());
            let few_required = config
                .map_max_required_keys
                .is_none_or(|max| required_key_count <= max);
            if may_be_map && few_required {
                if let Some(values) = self.unified_values(props_id, field) {
                    obj.shift_remove(kw::PROPERTIES);
                    obj.shift_remove(kw::REQUIRED);
                    let object = self.dag.string(kw::OBJECT);
                    obj.insert(kw::TYPE, object);
                    let values = self.rewrite_objects(values, None, false);
                    obj.insert(kw::ADDITIONAL_PROPERTIES, values);
                    return obj.build(&mut self.dag);
                }
            }
        }

        // A field forced to some other type is left as it is
        if forced.is_some() {
            return obj.build(&mut self.dag);
        }
        // --- Recurse into nested values ---
        self.rewrite_fields(&mut obj);
        for i in 0..obj.0.len() {
            let (k, v) = obj.0[i];
            if matches!(
                k,
                kw::PROPERTIES
                    | kw::ITEMS
                    | kw::TYPE
                    | kw::REQUIRED
                    | kw::SCHEMA
                    | kw::NAMESPACE
                    | kw::NAME
            ) {
                continue;
            }
            if self.dag.as_object(v).is_some() {
                obj.0[i].1 = self.rewrite_objects(v, Some(k), false);
            }
        }
        obj.build(&mut self.dag)
    }

    /// `rewrite_fields`
    fn rewrite_fields(&mut self, obj: &mut ObjBuf) {
        if let Some(entries) = obj
            .get(kw::PROPERTIES)
            .and_then(|p| self.dag.as_object(p))
            .map(Obj::to_vec)
        {
            let entries: Vec<(Sym, Id)> = entries
                .into_iter()
                .map(|(k, v)| (k, self.rewrite_objects(v, Some(k), false)))
                .collect();
            let props = self.dag.object(&entries);
            obj.insert(kw::PROPERTIES, props);
        }
        if let Some(items) = obj.get(kw::ITEMS) {
            let items = self.rewrite_objects(items, None, false);
            obj.insert(kw::ITEMS, items);
        }
    }

    /// `promote_forced_scalar`: the promoted schema, if promoted.
    fn promote_forced_scalar(&mut self, schema: Id, field: Option<Sym>) -> Option<Id> {
        let name = field?;
        let name_str = self.dag.name(name);
        if !self.is_forced_scalar(name_str) || self.config.force_field_types.contains_key(name_str)
        {
            return None;
        }
        let t = self.dag.get(schema, kw::TYPE)?;
        let scalar_type = match self.dag.term(t) {
            Term::String(t) => Some(t),
            Term::Array(types)
                if types.len() == 2 && types.iter().any(|&t| self.dag.is_sym(t, kw::NULL)) =>
            {
                types
                    .iter()
                    .copied()
                    .find(|&t| !self.dag.is_sym(t, kw::NULL))
                    .and_then(|t| self.dag.as_sym(t))
            }
            _ => None,
        }
        .filter(|t| matches!(*t, kw::STRING | kw::INTEGER | kw::NUMBER | kw::BOOLEAN))?;
        let key = self.promoted_key(name, scalar_type);
        Some(self.record_of(key, schema))
    }

    /// `identical_record_values`
    fn identical_record_values(&mut self, props: Id) -> Option<Id> {
        let values: Vec<Id> = self.dag.as_object(props)?.values().collect();
        let &first = values.first()?;
        let is_record = self
            .dag
            .get(first, kw::TYPE)
            .is_some_and(|t| self.dag.is_sym(t, kw::OBJECT))
            && self.dag.get(first, kw::PROPERTIES).is_some();
        (is_record && values.len() > 1 && values[1..].iter().all(|&v| self.dag.same(v, first)))
            .then_some(first)
    }

    /// `unified_values`
    fn unified_values(&mut self, props: Id, field: Option<Sym>) -> Option<Id> {
        let props_obj = self.dag.as_object(props)?;
        let children: Vec<Id> = props_obj.values().collect();
        let no_unify = self.in_no_unify(props_obj);
        let &first = children.first()?;
        let first = self.non_null(first);
        let homog_start = std::time::Instant::now();
        let homogeneous = children.iter().all(|&child| {
            let view = self.non_null(child);
            self.dag.same(view, first)
        });
        let mut unified = if homogeneous {
            Some(first)
        } else if !self.config.unify_maps || no_unify {
            None
        } else {
            let path = field.unwrap_or(kw::EMPTY);
            let unify_start = std::time::Instant::now();
            let all_arrays = children.iter().all(|&s| {
                self.dag
                    .get(s, kw::TYPE)
                    .is_some_and(|t| self.dag.is_sym(t, kw::ARRAY))
            });
            let unified = if all_arrays {
                // Arrays: their items are unified, and every array must have them
                let items: Option<Vec<Id>> = children
                    .iter()
                    .map(|&s| self.dag.get(s, kw::ITEMS))
                    .collect();
                items
                    .and_then(|items| self.check_unifiable_item_schemas(&items, path))
                    .map(|items| {
                        let array = self.dag.string(kw::ARRAY);
                        self.dag.object(&[(kw::TYPE, array), (kw::ITEMS, items)])
                    })
            } else {
                self.check_unifiable_schemas(&children, path)
            };
            if self.config.profile && children.len() > 50 {
                anstream::eprintln!(
                    "Unification of {} child schemas took {:?}",
                    children.len(),
                    unify_start.elapsed()
                );
            }
            unified
        };
        if self.config.profile && children.len() > 50 {
            anstream::eprintln!(
                "Homogeneity check on {} schemas took {:?}",
                children.len(),
                homog_start.elapsed()
            );
        }
        if let Some(schema) = unified.filter(|&schema| self.contains_anyof(schema)) {
            unified = Some(self.process_anyof_unions(schema, field.unwrap_or(kw::EMPTY)));
        }
        unified
    }

    /// `preprocess_force_field_types`
    pub(crate) fn preprocess_force_field_types(&mut self, schema: Id) -> Id {
        let config = self.config;
        match self.dag.term(schema) {
            Term::Object(obj) => {
                let mut obj = ObjBuf::of(obj);
                if let Some(entries) = obj
                    .get(kw::PROPERTIES)
                    .and_then(|p| self.dag.as_object(p))
                    .map(Obj::to_vec)
                {
                    let entries: Vec<(Sym, Id)> = entries
                        .into_iter()
                        .map(|(name, mut field_schema)| {
                            let forced = config.force_field_types.get(self.dag.name(name));
                            if forced.is_some_and(|f| f == "map") {
                                field_schema = self.convert_to_map(field_schema, name);
                            }
                            (name, self.preprocess_force_field_types(field_schema))
                        })
                        .collect();
                    let props = self.dag.object(&entries);
                    obj.insert(kw::PROPERTIES, props);
                }
                for keyword in [kw::ITEMS, kw::ADDITIONAL_PROPERTIES] {
                    if let Some(value) = obj.get(keyword) {
                        let value = self.preprocess_force_field_types(value);
                        obj.insert(keyword, value);
                    }
                }
                if let Some(any_of) = obj
                    .get(kw::ANY_OF)
                    .and_then(|a| self.dag.as_array(a))
                    .map(<[Id]>::to_vec)
                {
                    let any_of: Vec<Id> = any_of
                        .into_iter()
                        .map(|item| self.preprocess_force_field_types(item))
                        .collect();
                    let any_of = self.dag.array(&any_of);
                    obj.insert(kw::ANY_OF, any_of);
                }
                obj.build(&mut self.dag)
            }
            Term::Array(items) => {
                let items: Vec<Id> = items.to_vec();
                let items: Vec<Id> = items
                    .into_iter()
                    .map(|item| self.preprocess_force_field_types(item))
                    .collect();
                self.dag.array(&items)
            }
            _ => schema,
        }
    }

    /// `convert_to_map`
    fn convert_to_map(&mut self, schema: Id, field: Sym) -> Id {
        let null_object = |on: &mut Self| {
            let null = on.dag.string(kw::NULL);
            let object = on.dag.string(kw::OBJECT);
            on.dag.array(&[null, object])
        };
        if let Some(items) = self.dag.as_array(schema) {
            let has_null = items.iter().any(|&v| {
                self.dag.is_sym(v, kw::NULL)
                    || self
                        .dag
                        .get(v, kw::TYPE)
                        .is_some_and(|t| self.dag.is_sym(t, kw::NULL))
            });
            let t = if has_null {
                null_object(self)
            } else {
                self.dag.string(kw::OBJECT)
            };
            let string = self.typed(kw::STRING);
            return self
                .dag
                .object(&[(kw::TYPE, t), (kw::ADDITIONAL_PROPERTIES, string)]);
        }
        let Some(obj) = self.dag.as_object(schema) else {
            return schema;
        };
        let mut obj = ObjBuf::of(obj);
        if obj.contains(kw::ADDITIONAL_PROPERTIES) {
            return schema;
        }
        let is_nullable = obj
            .get(kw::TYPE)
            .and_then(|t| self.dag.as_array(t))
            .is_some_and(|ts| ts.iter().any(|&t| self.dag.is_sym(t, kw::NULL)));
        let values = self.forced_map_value_schema(&obj, Some(field));
        obj.shift_remove(kw::PROPERTIES);
        obj.shift_remove(kw::REQUIRED);
        obj.shift_remove(kw::ANY_OF);
        let t = if is_nullable {
            null_object(self)
        } else {
            self.dag.string(kw::OBJECT)
        };
        obj.insert(kw::TYPE, t);
        obj.insert(kw::ADDITIONAL_PROPERTIES, values);
        obj.build(&mut self.dag)
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/on_dag.rs");
}
