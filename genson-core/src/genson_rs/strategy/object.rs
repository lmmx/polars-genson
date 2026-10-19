use ordermap::OrderMap;
use regex::Regex;
use rustc_hash::FxBuildHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;

type PropMap<V> = OrderMap<String, V, FxBuildHasher>;
type KeySet<T> = HashSet<T, FxBuildHasher>;

use rayon::prelude::*;
use serde_json::{json, Map, Value};

use crate::dag::{kw, Dag, Id, Sym};
use crate::genson_rs::interned::{RowStates, StateId};
use crate::genson_rs::node::{hash_value, DataType, DeferredStrategy, SchemaNode, PAR_MERGE_MIN};
use crate::genson_rs::strategy::base::SchemaStrategy;
use crate::genson_rs::strategy::keywords;

#[derive(Debug, PartialEq)]
pub struct ObjectStrategy {
    // TODO: this is redeclared everywhere, how to avoid this?
    extra_keywords: Value,
    properties: PropMap<SchemaNode>,
    pattern_properties: PropMap<SchemaNode>,
    pattern_regexes: PatternRegexes,
    /// A map's value schema (`additionalProperties` as a schema), merged from every
    /// schema added like a property, not kept from the first as other keywords are
    additional_properties: Option<SchemaNode>,
    required_properties: Option<KeySet<String>>,
    include_empty_required: bool,
}

/// The compiled `patternProperties` patterns, in the order of `pattern_properties` (which
/// only grows), compiled when first matched against.
#[derive(Debug, Default)]
struct PatternRegexes(Vec<Regex>);

impl PatternRegexes {
    fn compiled(&mut self, patterns: &PropMap<SchemaNode>) -> &[Regex] {
        for pattern in patterns.keys().skip(self.0.len()) {
            self.0.push(Regex::new(pattern).unwrap());
        }
        &self.0
    }
}

/// Derived from `pattern_properties`, so it takes no part in comparing strategies.
impl PartialEq for PatternRegexes {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl ObjectStrategy {
    pub fn new() -> Self {
        ObjectStrategy {
            extra_keywords: json!({}),
            properties: PropMap::default(),
            pattern_properties: PropMap::default(),
            pattern_regexes: PatternRegexes::default(),
            additional_properties: None,
            required_properties: None,
            include_empty_required: false,
        }
    }
}

impl SchemaStrategy for ObjectStrategy {
    fn get_extra_keywords_mut(&mut self) -> &mut Value {
        &mut self.extra_keywords
    }

    fn get_extra_keywords(&self) -> &Value {
        &self.extra_keywords
    }

    /// Like the default, but `properties`, `patternProperties` and `required` only reserve
    /// their key position: `to_schema` overwrites or removes them, and a deep copy of the
    /// first sub-schema at every object level is quadratic in depth.
    fn add_extra_keywords(&mut self, schema: &Value) {
        let (Value::Object(schema), Value::Object(keywords)) = (schema, &mut self.extra_keywords)
        else {
            return;
        };
        for (key, value) in schema {
            if key == "type" || keywords.contains_key(key) {
                continue;
            }
            let value = match key.as_str() {
                // An empty map in the builder means the first one seen was empty
                "properties" | "patternProperties" | "additionalProperties"
                    if value.is_object() =>
                {
                    json!({})
                }
                "required" => Value::Null,
                _ => value.clone(),
            };
            keywords.insert(key.clone(), value);
        }
    }

    fn match_schema(schema: &Value) -> bool {
        schema["type"] == "object"
    }

    fn match_object(object: crate::genson_rs::JsonValue) -> bool {
        object.is_object()
    }

    fn add_object(&mut self, object: crate::genson_rs::JsonValue) {
        // Without pattern properties every key is a plain property, so skip the per-object
        // key set and the repeated lookups.
        if self.pattern_properties.is_empty() {
            if let Some(object) = object.as_object() {
                for (prop, subobj) in object.iter() {
                    if let Some(node) = self.properties.get_mut(prop) {
                        node.add_object(DataType::Object(subobj));
                    } else {
                        let mut node = SchemaNode::new();
                        node.add_object(DataType::Object(subobj));
                        self.properties.insert(prop.to_string(), node);
                    }
                }
                match &mut self.required_properties {
                    None => {
                        self.required_properties =
                            Some(object.keys().map(|p| p.to_string()).collect());
                    }
                    Some(req) => {
                        if !req.is_empty() {
                            let keys: KeySet<&str> = object.keys().collect();
                            req.retain(|p| keys.contains(p.as_str()));
                        }
                    }
                }
                return;
            }
        }

        let mut properties: KeySet<&str> = KeySet::default();
        if let Some(object) = object.as_object() {
            object.iter().for_each(|(prop, subobj)| {
                let mut matched = false;
                if !self.properties.contains_key(prop) {
                    let regexes = self.pattern_regexes.compiled(&self.pattern_properties);
                    if let Some((node, _)) = self
                        .pattern_properties
                        .values_mut()
                        .zip(regexes)
                        .find(|(_, regex)| regex.is_match(prop))
                    {
                        matched = true;
                        node.add_object(DataType::Object(subobj));
                    }
                }

                if !matched {
                    properties.insert(prop);
                    if !self.properties.contains_key(prop) {
                        self.properties.insert(prop.to_string(), SchemaNode::new());
                    }
                    self.properties
                        .get_mut(prop)
                        .unwrap()
                        .add_object(DataType::Object(subobj));
                }
            });
        }

        if self.required_properties.is_none() {
            self.required_properties = Some(properties.iter().map(|p| p.to_string()).collect());
        } else if let Some(req) = &mut self.required_properties {
            // take the intersection
            if !req.is_empty() {
                req.retain(|p| properties.contains(p.as_str()));
            }
        }
    }

    fn add_schema(&mut self, schema: &Value) {
        if let Value::Object(schema_object) = schema {
            self.add_extra_keywords(schema);

            // properties updater updates the internal properties and pattern_properties with the schema_object,
            // creating schema node as needed for each property
            let properties_updater = |properties: &mut PropMap<SchemaNode>,
                                      schema_object: &Map<String, Value>,
                                      prop_key: &str| {
                if let Some(schema_properties) = schema_object[prop_key].as_object() {
                    schema_properties.iter().for_each(|(prop, sub_schema)| {
                        if let Some(sub_node) = properties.get_mut(prop.as_str()) {
                            sub_node.add_schema(DataType::Schema(sub_schema));
                        } else {
                            let sub_node = properties.entry(prop.to_string()).or_default();
                            sub_node.add_schema(DataType::Schema(sub_schema));
                        }
                    });
                }
            };

            if schema_object.contains_key("properties") {
                properties_updater(&mut self.properties, schema_object, "properties");
            }
            if schema_object.contains_key("patternProperties") {
                properties_updater(
                    &mut self.pattern_properties,
                    schema_object,
                    "patternProperties",
                );
            }
            if let Some(values) = schema_object.get("additionalProperties") {
                if values.is_object() {
                    self.additional_properties
                        .get_or_insert_with(SchemaNode::new)
                        .add_schema(DataType::Schema(values));
                }
            }
            self.merge_required(schema_object);
        } else {
            panic!("Invalid schema type - must be a valid JSON object")
        }
    }

    /// Optimized batch schema merging for objects
    /// Collects all properties from all schemas first, then merges each property once
    fn add_schemas(&mut self, schemas: &[&Value]) {
        // Phase 1: Collect all properties and required sets from all schemas
        let mut property_groups: PropMap<Vec<&Value>> = PropMap::default();
        let mut pattern_property_groups: PropMap<Vec<&Value>> = PropMap::default();
        let mut additional_properties: Vec<&Value> = Vec::new();
        let mut all_required_sets: Vec<KeySet<String>> = Vec::new();

        for schema in schemas {
            if let Value::Object(schema_obj) = schema {
                self.add_extra_keywords(schema);

                // Collect regular properties
                if let Some(Value::Object(props)) = schema_obj.get("properties") {
                    for (prop_name, sub_schema) in props {
                        property_groups
                            .entry(prop_name.clone())
                            .or_default()
                            .push(sub_schema);
                    }
                }

                // Collect pattern properties
                if let Some(Value::Object(patterns)) = schema_obj.get("patternProperties") {
                    for (pattern, sub_schema) in patterns {
                        pattern_property_groups
                            .entry(pattern.clone())
                            .or_default()
                            .push(sub_schema);
                    }
                }

                // Collect map value schemas
                if let Some(values) = schema_obj.get("additionalProperties") {
                    if values.is_object() {
                        additional_properties.push(values);
                    }
                }

                // Collect required fields
                if let Some(Value::Array(required)) = schema_obj.get("required") {
                    if required.is_empty() {
                        self.include_empty_required = true;
                    }
                    let required_set: KeySet<String> = required
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    all_required_sets.push(required_set);
                }
            }
        }

        // Phase 2: Merge properties in parallel if there are enough properties
        if property_groups.len() > 3 {
            // Each property's schemas merge into the node it already has (one is added, in
            // order, where it has none), so that a property held before keeps its schema
            for prop_name in property_groups.keys() {
                if !self.properties.contains_key(prop_name) {
                    self.properties.insert(prop_name.clone(), SchemaNode::new());
                }
            }
            let mut groups: HashMap<String, Vec<&Value>, FxBuildHasher> =
                property_groups.into_iter().collect();
            let work: Vec<(&mut SchemaNode, Vec<&Value>)> = self
                .properties
                .iter_mut()
                .filter_map(|(name, node)| groups.remove(name).map(|s| (node, s)))
                .collect();
            work.into_par_iter().for_each(|(node, sub_schemas)| {
                node.add_schemas(&sub_schemas);
            });
        } else {
            // Sequential for small property counts
            for (prop_name, sub_schemas) in property_groups {
                let node = self.properties.entry(prop_name).or_default();
                node.add_schemas(&sub_schemas);
            }
        }

        // Phase 3: Merge pattern properties (usually few, so sequential is fine)
        for (pattern, sub_schemas) in pattern_property_groups {
            let node = self.pattern_properties.entry(pattern).or_default();
            node.add_schemas(&sub_schemas);
        }

        // Phase 3b: Merge map value schemas
        if !additional_properties.is_empty() {
            self.additional_properties
                .get_or_insert_with(SchemaNode::new)
                .add_schemas(&additional_properties);
        }

        // Phase 4: Merge required fields (intersection of all sets)
        if !all_required_sets.is_empty() {
            let final_required = all_required_sets
                .into_iter()
                .reduce(|acc, set| acc.intersection(&set).cloned().collect())
                .unwrap_or_default();

            if self.required_properties.is_none() {
                self.required_properties = Some(final_required);
            } else if let Some(req) = &mut self.required_properties {
                req.retain(|p| final_required.contains(p));
            }
        }
    }

    fn to_schema(&self) -> Value {
        let mut schema = self.extra_keywords.clone();
        schema["type"] = "object".into();
        if !self.properties.is_empty() {
            schema["properties"] = self.properties_to_schema(&self.properties);
        }
        if !self.pattern_properties.is_empty() {
            schema["patternProperties"] = self.properties_to_schema(&self.pattern_properties);
        }
        if let Some(values) = &self.additional_properties {
            schema["additionalProperties"] = values.to_schema();
        }
        if self.required_properties.is_some() || self.include_empty_required {
            let mut required_props: Vec<String>;
            if let Some(required_properties) = &self.required_properties {
                required_props = required_properties.iter().map(|p| p.to_string()).collect();
            } else {
                required_props = vec![];
            }
            required_props.sort();

            if !required_props.is_empty() || self.include_empty_required {
                schema["required"] = required_props.into();
            } else {
                // this is done in case there's a conflict with the required properties
                // from extra keywords
                schema.as_object_mut().unwrap().shift_remove("required");
            }
        } else {
            schema.as_object_mut().unwrap().shift_remove("required");
        }
        schema
    }
}

impl ObjectStrategy {
    /// Whether `to_schema` writes `required`: when some key is required, or an empty
    /// `required` was asked for.
    fn writes_required(&self) -> bool {
        self.include_empty_required
            || self
                .required_properties
                .as_ref()
                .is_some_and(|r| !r.is_empty())
    }

    /// Whether `to_schema` writes its own value for `key`, in place of any extra keyword.
    fn writes(&self, key: &str) -> bool {
        match key {
            "properties" => !self.properties.is_empty(),
            "patternProperties" => !self.pattern_properties.is_empty(),
            "additionalProperties" => self.additional_properties.is_some(),
            "required" => self.writes_required(),
            _ => false,
        }
    }

    /// The keys of `to_schema`'s output besides `type`, in their order: the extra keywords,
    /// then the structural keywords written that are not among them.
    fn schema_keys(&self) -> Vec<&str> {
        let Value::Object(keywords) = &self.extra_keywords else {
            unreachable!("extra keywords are always an object")
        };
        let written = [
            "properties",
            "patternProperties",
            "additionalProperties",
            "required",
        ]
        .into_iter()
        .filter(|key| self.writes(key) && !keywords.contains_key(*key));
        keywords
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "required" || self.writes_required())
            .chain(written)
            .collect()
    }

    /// Whether `to_schema` gives nothing but `{"type": "object"}`.
    pub(crate) fn is_bare(&self) -> bool {
        self.schema_keys().is_empty()
    }

    /// Merge `other` into this strategy, with the same result as
    /// `self.add_schema(&other.to_schema())` (but see `SchemaNode::absorb` on `required`):
    /// properties new to this strategy are moved in, the rest merge node by node.
    pub(crate) fn absorb(&mut self, mut other: ObjectStrategy) {
        self.absorb_keywords_and_required(&mut other);
        absorb_properties(&mut self.properties, other.properties);
        absorb_properties(&mut self.pattern_properties, other.pattern_properties);
        if let Some(values) = other.additional_properties {
            match &mut self.additional_properties {
                Some(own) => own.absorb(values),
                None => self.additional_properties = Some(SchemaNode::round_tripped(values)),
            }
        }
    }

    /// `absorb` of the object an interned state stands for: the keywords its schema writes
    /// (`properties`, and `required` where it requires a key), the intersection of the
    /// `required` sets, and its properties, merged into the same-named nodes or added
    /// round-tripped (absorbed into a new node), as `absorb_properties` adds them.
    pub(crate) fn absorb_interned(&mut self, rows: &RowStates, obj: u32) {
        self.absorb_interned_keywords(rows, obj);
        for &(key, state) in rows.obj(obj).props.iter() {
            let name = rows.key(key);
            match self.properties.get_mut(name) {
                Some(node) => node.absorb_interned(rows, state),
                None => {
                    let mut node = SchemaNode::new();
                    node.absorb_interned(rows, state);
                    self.properties.insert(name.to_string(), node);
                }
            }
        }
    }

    /// `absorb_interned`, with each property's absorb added to `deferred`: a new property's
    /// node is added (empty) where `absorb_interned` adds it, and its states absorbed later.
    pub(crate) fn absorb_interned_deferred<'a>(
        &mut self,
        rows: &'a RowStates,
        obj: u32,
        deferred: &mut DeferredStrategy<'a>,
    ) {
        self.absorb_interned_keywords(rows, obj);
        if !matches!(deferred, DeferredStrategy::Object(_)) {
            *deferred = DeferredStrategy::Object(Vec::new());
        }
        let DeferredStrategy::Object(groups) = deferred else {
            unreachable!()
        };
        for &(key, state) in rows.obj(obj).props.iter() {
            let name = rows.key(key);
            let at = match self.properties.get_index_of(name) {
                Some(at) => at,
                None => {
                    self.properties.insert(name.to_string(), SchemaNode::new());
                    self.properties.len() - 1
                }
            };
            if groups.len() <= at {
                groups.resize_with(at + 1, Vec::new);
            }
            groups[at].push((rows, state));
        }
    }

    /// The property absorbs `absorb_interned_deferred` held back, each property's node
    /// taking its states in their order, the properties on their own threads.
    pub(crate) fn absorb_deferred(&mut self, groups: Vec<Vec<(&RowStates, StateId)>>) {
        let work: Vec<(&mut SchemaNode, Vec<(&RowStates, StateId)>)> = self
            .properties
            .values_mut()
            .zip(groups)
            .filter(|(_, states)| !states.is_empty())
            .collect();
        if work.len() > 1
            && work.iter().map(|(_, states)| states.len()).sum::<usize>() >= PAR_MERGE_MIN
        {
            work.into_par_iter()
                .for_each(|(node, states)| node.absorb_interned_all(&states));
        } else {
            for (node, states) in work {
                node.absorb_interned_all(&states);
            }
        }
    }

    /// The part of `absorb_interned` besides the child nodes: the keyword placeholders and
    /// the intersection of the `required` sets.
    fn absorb_interned_keywords(&mut self, rows: &RowStates, obj: u32) {
        let other = rows.obj(obj);
        if let Value::Object(own) = &mut self.extra_keywords {
            // A strategy holds a few keywords: looked for in turn, not by hash
            let has = |own: &Map<String, Value>, key: &str| own.keys().any(|k| k == key);
            if !other.props.is_empty() && !has(own, "properties") {
                own.insert("properties".to_string(), json!({}));
            }
            if !other.required.is_empty() && !has(own, "required") {
                own.insert("required".to_string(), Value::Null);
            }
        }
        let required = || {
            other
                .required
                .iter()
                .map(move |&i| rows.key(other.props[i as usize].0))
        };
        match &mut self.required_properties {
            None => self.required_properties = Some(required().map(str::to_string).collect()),
            Some(own) if !own.is_empty() => {
                let keys: KeySet<&str> = required().collect();
                own.retain(|p| keys.contains(p.as_str()));
            }
            Some(_) => {}
        }
    }

    /// `absorb` each of `others` in turn, with each property's nodes merged on its own
    /// thread (the properties are independent subtrees, and each still takes its nodes in
    /// order).
    pub(crate) fn absorb_all(&mut self, others: Vec<ObjectStrategy>) {
        let mut groups: HashMap<String, Vec<SchemaNode>, FxBuildHasher> = HashMap::default();
        let mut values: Vec<SchemaNode> = Vec::new();
        for mut other in others {
            self.absorb_keywords_and_required(&mut other);
            for (name, node) in other.properties {
                if !self.properties.contains_key(&name) {
                    // Absorbed into a new node, as `absorb` would round-trip it
                    self.properties.insert(name.clone(), SchemaNode::new());
                }
                groups.entry(name).or_default().push(node);
            }
            absorb_properties(&mut self.pattern_properties, other.pattern_properties);
            values.extend(other.additional_properties);
        }
        let work: Vec<(&mut SchemaNode, Vec<SchemaNode>)> = self
            .properties
            .iter_mut()
            .filter_map(|(name, node)| groups.remove(name).map(|nodes| (node, nodes)))
            .collect();
        work.into_par_iter()
            .for_each(|(node, nodes)| node.absorb_all(nodes));
        if !values.is_empty() {
            self.additional_properties
                .get_or_insert_with(SchemaNode::new)
                .absorb_all(values);
        }
    }

    /// The part of `absorb` besides the child nodes: the keywords `add_extra_keywords`
    /// records from `other`'s schema, and the intersection of the `required` sets.
    fn absorb_keywords_and_required(&mut self, other: &mut ObjectStrategy) {
        if let Value::Object(own) = &mut self.extra_keywords {
            // Only a key this strategy has not recorded is added, so only those are built
            for key in other.schema_keys() {
                if own.contains_key(key) {
                    continue;
                }
                let placeholder = match key {
                    "properties" | "patternProperties" | "additionalProperties"
                        if other.writes(key) || other.extra_keywords[key].is_object() =>
                    {
                        json!({})
                    }
                    "required" => Value::Null,
                    _ => other.extra_keywords[key].clone(),
                };
                own.insert(key.to_string(), placeholder);
            }
        }

        // The keys present in every object `other` has seen: those are what limits this
        // strategy's `required`, including when there are none (which `to_schema` does not
        // write, so merging the schema value would take no account of it)
        let required = match other.required_properties.take() {
            Some(required) => required,
            None if other.include_empty_required => KeySet::default(),
            None => return,
        };
        if required.is_empty() && other.include_empty_required {
            self.include_empty_required = true;
        }
        match &mut self.required_properties {
            None => self.required_properties = Some(required),
            Some(own) => own.retain(|p| required.contains(p)),
        }
    }

    /// See `SchemaNode::visit_properties_mut`.
    pub(crate) fn visit_properties_mut<F>(&mut self, f: &mut F)
    where
        F: FnMut(&str, &mut SchemaNode) -> bool,
    {
        for (name, node) in self.properties.iter_mut() {
            if f(name, node) {
                node.visit_properties_mut(f);
            }
        }
        if let Some(values) = &mut self.additional_properties {
            values.visit_properties_mut(f);
        }
    }

    /// Feed `hasher` what identifies `to_schema`'s output, without building it: the
    /// keywords in order, with the child nodes' own hashes in place of their schemas.
    pub(crate) fn hash_schema<H: Hasher>(&self, hasher: &mut H) {
        for key in self.schema_keys() {
            hasher.write(key.as_bytes());
            hasher.write_u8(0xff);
            match key {
                "properties" | "patternProperties" if self.writes(key) => {
                    let properties = if key == "properties" {
                        &self.properties
                    } else {
                        &self.pattern_properties
                    };
                    hasher.write_usize(properties.len());
                    for (name, node) in properties {
                        hasher.write(name.as_bytes());
                        hasher.write_u8(0xff);
                        node.hash_schema(hasher);
                    }
                }
                "additionalProperties" if self.writes(key) => {
                    if let Some(values) = &self.additional_properties {
                        values.hash_schema(hasher);
                    }
                }
                "required" => {
                    let mut required: Vec<&str> = self
                        .required_properties
                        .iter()
                        .flatten()
                        .map(String::as_str)
                        .collect();
                    required.sort_unstable();
                    hasher.write_usize(required.len());
                    for name in required {
                        hasher.write(name.as_bytes());
                        hasher.write_u8(0xff);
                    }
                }
                _ => hash_value(&self.extra_keywords[key], hasher),
            }
        }
    }

    /// Intersect the `required` set with the schema object's `required` array.
    fn merge_required(&mut self, schema_object: &Map<String, Value>) {
        if schema_object.contains_key("required") {
            if let Value::Array(required_fields) = &schema_object["required"] {
                if required_fields.is_empty() {
                    // if the input schema object has required fields being empty, that means
                    // including empty required fields in the schema is the desired behavior
                    // and should be followed
                    self.include_empty_required = true;
                }
                if self.required_properties.is_none() {
                    let required_fields_set: KeySet<String> = required_fields
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect();
                    self.required_properties = Some(required_fields_set);
                } else if let Some(req) = &mut self.required_properties {
                    // take the intersection
                    let incoming: KeySet<&str> =
                        required_fields.iter().filter_map(|v| v.as_str()).collect();
                    req.retain(|p| incoming.contains(p.as_str()));
                }
            }
        }
    }

    /// Merge `schemas` in order, same result as calling `add_schema` on each in turn, but
    /// with each property's node merged on its own thread (the properties are independent
    /// subtrees, and every node still sees its sub-schemas in order).
    pub(crate) fn add_schemas_par(&mut self, schemas: &[&Value]) {
        if schemas
            .iter()
            .any(|schema| schema.get("patternProperties").is_some())
        {
            schemas.iter().for_each(|schema| self.add_schema(schema));
            return;
        }

        let mut groups: HashMap<&str, Vec<&Value>, FxBuildHasher> = HashMap::default();
        let mut additional_properties: Vec<&Value> = Vec::new();
        for schema in schemas {
            let Value::Object(schema_object) = schema else {
                panic!("Invalid schema type - must be a valid JSON object")
            };
            self.add_extra_keywords(schema);
            if let Some(Value::Object(props)) = schema_object.get("properties") {
                for (prop, sub_schema) in props {
                    if !self.properties.contains_key(prop.as_str()) {
                        self.properties.insert(prop.clone(), SchemaNode::new());
                    }
                    groups.entry(prop.as_str()).or_default().push(sub_schema);
                }
            }
            if let Some(values) = schema_object.get("additionalProperties") {
                if values.is_object() {
                    additional_properties.push(values);
                }
            }
            self.merge_required(schema_object);
        }
        if !additional_properties.is_empty() {
            self.additional_properties
                .get_or_insert_with(SchemaNode::new)
                .add_schemas_par(&additional_properties);
        }

        self.properties.par_iter_mut().for_each(|(prop, node)| {
            if let Some(sub_schemas) = groups.get(prop.as_str()) {
                node.add_schemas_par(sub_schemas);
            }
        });
    }
}

/// Merge each of `other`'s property nodes into the same-named one in `properties`, or add it
/// (round-tripped, as `add_schema` would build it) after the existing ones.
fn absorb_properties(properties: &mut PropMap<SchemaNode>, other: PropMap<SchemaNode>) {
    for (name, node) in other {
        match properties.get_mut(&name) {
            Some(own) => own.absorb(node),
            None => {
                properties.insert(name, SchemaNode::round_tripped(node));
            }
        }
    }
}

/// Properties from which an object writes its properties' schemas in parallel.
const PAR_TO_SCHEMA_MIN: usize = 16;

impl ObjectStrategy {
    /// `to_schema` as a DAG.
    pub(crate) fn to_dag(&self, dag: &mut Dag) -> Id {
        let mut schema = keywords(&self.extra_keywords, dag);
        let object = dag.string(kw::OBJECT);
        schema.insert(kw::TYPE, object);
        if !self.properties.is_empty() {
            let properties = Self::properties_to_dag(&self.properties, dag);
            schema.insert(kw::PROPERTIES, properties);
        }
        if !self.pattern_properties.is_empty() {
            let patterns = Self::properties_to_dag(&self.pattern_properties, dag);
            schema.insert(kw::PATTERN_PROPERTIES, patterns);
        }
        if let Some(values) = &self.additional_properties {
            let values = values.to_dag(dag);
            schema.insert(kw::ADDITIONAL_PROPERTIES, values);
        }
        let mut required: Vec<&str> = self
            .required_properties
            .iter()
            .flatten()
            .map(String::as_str)
            .collect();
        required.sort();
        if !required.is_empty() || self.include_empty_required {
            let names: Vec<Id> = required.iter().map(|name| dag.str(name)).collect();
            let required = dag.array(&names);
            schema.insert(kw::REQUIRED, required);
        } else {
            schema.shift_remove(kw::REQUIRED);
        }
        schema.build(dag)
    }

    fn properties_to_dag(properties: &PropMap<SchemaNode>, dag: &mut Dag) -> Id {
        let entries: Vec<(Sym, Id)> = properties
            .iter()
            .map(|(name, node)| (dag.sym(name), node.to_dag(dag)))
            .collect();
        dag.object(&entries)
    }

    fn properties_to_schema(&self, properties: &PropMap<SchemaNode>) -> Value {
        // Each property's schema is written on its own, so many are written in parallel
        // (in order)
        let schemas: Vec<Value> = if properties.len() >= PAR_TO_SCHEMA_MIN {
            properties
                .par_iter()
                .map(|(_, node)| node.to_schema())
                .collect()
        } else {
            properties.values().map(SchemaNode::to_schema).collect()
        };
        let mut schema_properties = Map::with_capacity(properties.len());
        for (prop, schema) in properties.keys().zip(schemas) {
            schema_properties.insert(prop.clone(), schema);
        }
        Value::Object(schema_properties)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_objects(strategy: &mut ObjectStrategy, objects: &[&str]) {
        for object in objects {
            let mut bytes = object.as_bytes().to_vec();
            let tape = simd_json::to_tape(&mut bytes).unwrap();
            strategy.add_object(tape.as_value());
        }
    }

    /// Keys matching a `patternProperties` pattern merge into that pattern's node (the
    /// first pattern matching, in the order the patterns were added), the rest into
    /// `properties`, and a pattern added after some objects applies to the objects after it.
    #[test]
    fn test_pattern_properties_take_matching_keys() {
        let mut strategy = ObjectStrategy::new();
        strategy.add_schema(&json!({
            "type": "object",
            "patternProperties": {
                "^P[0-9]+$": {"type": "array", "items": {"type": "integer"}},
                "^P": {"type": "string"}
            }
        }));
        add_objects(&mut strategy, &[r#"{"P31": [1], "label": "x"}"#]);
        strategy.add_schema(&json!({
            "type": "object",
            "patternProperties": {"^Q[0-9]+$": {"type": "boolean"}}
        }));
        add_objects(
            &mut strategy,
            &[r#"{"P279": [2], "Px": "y", "Q5": true, "label": "z"}"#],
        );
        let schema = strategy.to_schema();
        assert_eq!(
            schema["patternProperties"],
            json!({
                "^P[0-9]+$": {"type": "array", "items": {"type": "integer"}},
                "^P": {"type": "string"},
                "^Q[0-9]+$": {"type": "boolean"}
            })
        );
        assert_eq!(schema["properties"], json!({"label": {"type": "string"}}));
        assert_eq!(schema["required"], json!(["label"]));
    }

    #[test]
    fn test_ordermap_equality_differs_from_indexmap() {
        // Two ObjectStrategies with same keys/values in different order
        let mut strat1 = ObjectStrategy::new();
        strat1.properties.insert("a".to_string(), SchemaNode::new());
        strat1.properties.insert("b".to_string(), SchemaNode::new());

        let mut strat2 = ObjectStrategy::new();
        strat2.properties.insert("b".to_string(), SchemaNode::new());
        strat2.properties.insert("a".to_string(), SchemaNode::new());

        let keys1: Vec<_> = strat1.properties.keys().collect();
        let keys2: Vec<_> = strat2.properties.keys().collect();
        anstream::println!("keys1 = {:?}", keys1);
        anstream::println!("keys2 = {:?}", keys2);

        // The keys are the same set, but in different order.
        // With OrderMap, properties != properties because order matters.
        // With IndexMap, they would compare equal.
        assert_ne!(
            strat1.properties, strat2.properties,
            "OrderMap should treat maps with different insertion order as unequal"
        );
    }
}
