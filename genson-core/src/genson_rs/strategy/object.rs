use ordermap::OrderMap;
use regex::Regex;
use rustc_hash::FxBuildHasher;
use std::collections::{HashMap, HashSet};

type PropMap<V> = OrderMap<String, V, FxBuildHasher>;
type KeySet<T> = HashSet<T, FxBuildHasher>;

use rayon::prelude::*;
use serde_json::{json, Map, Value};

use crate::genson_rs::node::{DataType, SchemaNode};
use crate::genson_rs::strategy::base::SchemaStrategy;

#[derive(Debug, PartialEq)]
pub struct ObjectStrategy {
    // TODO: this is redeclared everywhere, how to avoid this?
    extra_keywords: Value,
    properties: PropMap<SchemaNode>,
    pattern_properties: PropMap<SchemaNode>,
    required_properties: Option<KeySet<String>>,
    include_empty_required: bool,
}

impl ObjectStrategy {
    pub fn new() -> Self {
        ObjectStrategy {
            extra_keywords: json!({}),
            properties: PropMap::default(),
            pattern_properties: PropMap::default(),
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
                "properties" | "patternProperties" if value.is_object() => json!({}),
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
                let mut pattern: Option<&str> = None;
                if !self.properties.contains_key(prop) {
                    let pattern_matcher = |p: &str| Regex::new(p).unwrap().is_match(prop);
                    if let Some((p, node)) = self
                        .pattern_properties
                        .iter_mut()
                        .find(|(p, _)| pattern_matcher(p))
                    {
                        pattern = Some(p);
                        node.add_object(DataType::Object(subobj));
                    }
                }

                if pattern.is_none() {
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
            let merged_properties: Vec<(String, SchemaNode)> = property_groups
                .into_par_iter()
                .map(|(prop_name, sub_schemas)| {
                    let mut node = SchemaNode::new();
                    // Batch add all schemas for this property
                    let schema_values: Vec<Value> =
                        sub_schemas.iter().map(|&s| s.clone()).collect();
                    node.add_schemas(&schema_values);
                    (prop_name, node)
                })
                .collect();

            // Insert merged properties back
            for (prop_name, node) in merged_properties {
                self.properties.insert(prop_name, node);
            }
        } else {
            // Sequential for small property counts
            for (prop_name, sub_schemas) in property_groups {
                let node = self.properties.entry(prop_name).or_default();

                let schema_values: Vec<Value> = sub_schemas.iter().map(|&s| s.clone()).collect();
                node.add_schemas(&schema_values);
            }
        }

        // Phase 3: Merge pattern properties (usually few, so sequential is fine)
        for (pattern, sub_schemas) in pattern_property_groups {
            let node = self.pattern_properties.entry(pattern).or_default();

            let schema_values: Vec<Value> = sub_schemas.iter().map(|&s| s.clone()).collect();
            node.add_schemas(&schema_values);
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
            self.merge_required(schema_object);
        }

        self.properties.par_iter_mut().for_each(|(prop, node)| {
            if let Some(sub_schemas) = groups.get(prop.as_str()) {
                node.add_schemas_par(sub_schemas);
            }
        });
    }
}

impl ObjectStrategy {
    fn properties_to_schema(&self, properties: &PropMap<SchemaNode>) -> Value {
        let mut schema_properties = Map::with_capacity(properties.len());
        properties.iter().for_each(|(prop, node)| {
            schema_properties.insert(prop.clone(), node.to_schema());
        });
        Value::Object(schema_properties)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
