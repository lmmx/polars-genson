use core::panic;
use std::borrow::Cow;

use crate::genson_rs::strategy::base::SchemaStrategy;
use crate::genson_rs::strategy::scalar::TypelessStrategy;
use crate::genson_rs::strategy::BasicSchemaStrategy;
use ordermap::OrderMap;
use serde_json::{json, Value};

/// Fewest schemas merged into one node at once for which going parallel below it pays off.
const PAR_MERGE_MIN: usize = 8;

/// Basic schema generator class. SchemaNode objects can be loaded
/// up with existing schemas and objects before being serialized.
#[derive(Debug, PartialEq)]
pub struct SchemaNode {
    active_strategies: Vec<BasicSchemaStrategy>,
}

/// DataType wraps around different types of schema data that can be added
/// to a SchemaNode. It wraps references to Value objects and SchemaNode
/// objects so when it gets dropped the underlying data is not dropped.
#[derive(Clone)]
pub enum DataType<'a> {
    /// SchemaNode represents a JSON schema
    Schema(&'a Value),
    /// Object represents a valid JSON object (array, object, string, number, boolean, null)
    Object(&'a simd_json::BorrowedValue<'a>),
    /// SchemaNode reference
    SchemaNode(&'a SchemaNode),
}

impl Default for SchemaNode {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaNode {
    pub fn new() -> Self {
        SchemaNode {
            active_strategies: vec![],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.active_strategies.is_empty()
    }

    pub fn add_schema(&mut self, data: DataType) -> &mut Self {
        let schema: Cow<Value> = match data {
            DataType::SchemaNode(node) => Cow::Owned(node.to_schema()),
            DataType::Schema(schema) => Cow::Borrowed(schema),
            _ => panic!("Invalid schema type"),
        };

        // Common case: no `anyOf` / type array to split, so skip `get_subschemas` (which
        // allocates a Vec per node visited).
        if !SchemaNode::needs_splitting(&schema) {
            let active_strategy = self.get_or_create_strategy_for_schema(&schema);
            SchemaNode::add_schema_or_object_to_strategy(
                active_strategy,
                DataType::Schema(&schema),
            );
            return self;
        }

        for subschema in SchemaNode::get_subschemas(&schema) {
            let active_strategy = self.get_or_create_strategy_for_schema(&subschema);
            SchemaNode::add_schema_or_object_to_strategy(
                active_strategy,
                DataType::Schema(&subschema),
            );
        }
        self
    }

    fn needs_splitting(schema: &Value) -> bool {
        match schema {
            Value::Object(obj) => {
                matches!(obj.get("anyOf"), Some(Value::Array(_)))
                    || matches!(obj.get("type"), Some(Value::Array(_)))
            }
            _ => false,
        }
    }

    /// Merge `schemas` in order like repeated `add_schema`, going parallel below this node
    /// (across properties, and through arrays into their items) when there are enough of them
    /// to be worth it.
    pub fn add_schemas_par(&mut self, schemas: &[&Value]) {
        if schemas.len() >= PAR_MERGE_MIN
            && (self.add_object_schemas_par(schemas) || self.add_list_schemas_par(schemas))
        {
            return;
        }
        for schema in schemas {
            self.add_schema(DataType::Schema(schema));
        }
    }

    /// Like `add_object_schemas_par` for plain array schemas whose `items` is an object schema.
    fn add_list_schemas_par(&mut self, schemas: &[&Value]) -> bool {
        let plain_lists = schemas.iter().all(|schema| {
            schema["type"] == "array"
                && schema["items"].is_object()
                && !SchemaNode::needs_splitting(schema)
        });
        if !plain_lists {
            return false;
        }
        match self.active_strategies.as_slice() {
            [] => {
                self.get_or_create_strategy_for_schema(schemas[0]);
            }
            [BasicSchemaStrategy::List(_)] => {}
            _ => return false,
        }
        match self.active_strategies.first_mut() {
            Some(BasicSchemaStrategy::List(strategy)) => {
                let items: Vec<&Value> = schemas.iter().map(|schema| &schema["items"]).collect();
                strategy.add_item_schemas_par(&items);
                true
            }
            _ => false,
        }
    }

    /// Merge `schemas` in order like repeated `add_schema`, but in parallel across the
    /// properties when they are all plain object schemas merging into an object node. Returns
    /// false, having merged nothing, when they are not.
    pub fn add_object_schemas_par(&mut self, schemas: &[&Value]) -> bool {
        if schemas.is_empty() {
            return true;
        }
        let plain_objects = schemas
            .iter()
            .all(|schema| schema["type"] == "object" && !SchemaNode::needs_splitting(schema));
        if !plain_objects {
            return false;
        }
        match self.active_strategies.as_slice() {
            [] => {
                self.get_or_create_strategy_for_schema(schemas[0]);
            }
            [BasicSchemaStrategy::Object(_)] => {}
            _ => return false,
        }
        match self.active_strategies.first_mut() {
            Some(BasicSchemaStrategy::Object(strategy)) => {
                strategy.add_schemas_par(schemas);
                true
            }
            _ => false,
        }
    }

    /// Add multiple schemas at once with optimized batch processing
    pub fn add_schemas(&mut self, schemas: &[Value]) -> &mut Self {
        // Store owned subschemas to avoid lifetime issues
        let mut all_subschemas: Vec<Cow<Value>> = Vec::new();
        // Map strategy index to indices into all_subschemas
        let mut schema_groups: OrderMap<usize, Vec<usize>> = OrderMap::new();

        for schema in schemas {
            // Get all subschemas (handles anyOf, type arrays, etc.)
            for subschema in SchemaNode::get_subschemas(schema) {
                // Find or create strategy for this subschema
                let strategy_idx =
                    if let Some(idx) = self.get_strategy_for_kind(DataType::Schema(&subschema)) {
                        idx
                    } else {
                        // Create new strategy
                        if let Some(_strategy) =
                            self.create_strategy_for_kind(DataType::Schema(&subschema))
                        {
                            self.active_strategies.len() - 1
                        } else {
                            continue; // Skip if can't create strategy
                        }
                    };

                let subschema_idx = all_subschemas.len();
                all_subschemas.push(subschema);

                schema_groups
                    .entry(strategy_idx)
                    .or_default()
                    .push(subschema_idx);
            }
        }

        // Batch process schemas for each strategy
        for (strategy_idx, subschema_indices) in schema_groups {
            if let Some(strategy) = self.active_strategies.get_mut(strategy_idx) {
                // Gather references to the subschemas for this strategy
                let schema_refs: Vec<&Value> = subschema_indices
                    .iter()
                    .map(|&idx| all_subschemas[idx].as_ref())
                    .collect();
                strategy.add_schemas(&schema_refs);
            }
        }

        self
    }

    fn get_subschemas(schema: &Value) -> Vec<Cow<'_, Value>> {
        if let Value::Object(obj) = schema {
            if let Some(Value::Array(anyof)) = obj.get("anyOf") {
                return anyof.iter().flat_map(SchemaNode::get_subschemas).collect();
            } else if let Some(Value::Array(types)) = obj.get("type") {
                return types
                    .iter()
                    .map(|t| {
                        let mut new_schema = obj.clone();
                        new_schema["type"] = t.clone();
                        Cow::Owned(Value::Object(new_schema))
                    })
                    .collect();
            } else {
                return vec![Cow::Borrowed(schema)];
            }
        }
        vec![Cow::Borrowed(schema)]
    }

    /// Modify the schema to accomodate the object.
    pub fn add_object(&mut self, data: DataType) -> &mut Self {
        let object = match data {
            DataType::Object(obj) => obj,
            _ => panic!("Invalid object type"),
        };

        let active_strategy = self.get_or_create_strategy_for_object(object);
        SchemaNode::add_schema_or_object_to_strategy(active_strategy, DataType::Object(object));
        self
    }

    /// Convert the current schema node to a JSON schema
    pub fn to_schema(&self) -> Value {
        // Scalar strategies produce `{"type": t}`; keep those apart so they can be
        // collapsed into one entry (a single type, or a sorted type list) at the end.
        let mut scalar_schemas: Vec<Value> = vec![];
        let mut generated_schemas: Vec<Value> = vec![];

        self.active_strategies.iter().for_each(|strategy| {
            let generated_schema: Value = strategy.to_schema();
            if let Value::Object(ref schema) = generated_schema {
                // if schema is scalar type
                if schema.keys().len() == 1 && schema.contains_key("type") {
                    scalar_schemas.push(generated_schema);
                } else {
                    generated_schemas.push(generated_schema);
                }
            } else {
                panic!("Invalid schema type for strategy {:?}", strategy);
            }
        });

        if scalar_schemas.len() == 1 {
            generated_schemas.push(scalar_schemas.swap_remove(0));
        } else if !scalar_schemas.is_empty() {
            let mut scalar_type_list: Vec<&str> = scalar_schemas
                .iter()
                .map(|schema| schema["type"].as_str().unwrap())
                .collect();
            scalar_type_list.sort();
            scalar_type_list.dedup();
            if scalar_type_list.len() == 1 {
                generated_schemas.push(scalar_schemas.swap_remove(0));
            } else {
                generated_schemas.push(json!({"type": scalar_type_list}));
            }
        }

        if generated_schemas.len() == 1 {
            generated_schemas.swap_remove(0)
        } else if !generated_schemas.is_empty() {
            json!({"anyOf": generated_schemas})
        } else {
            json!({})
        }
    }

    /// Get the current active strategy for the object, if not found create a new one.
    fn get_or_create_strategy_for_object(
        &mut self,
        object: &simd_json::BorrowedValue,
    ) -> &mut BasicSchemaStrategy {
        if let Some(idx) = self.get_strategy_for_kind(DataType::Object(object)) {
            return &mut self.active_strategies[idx];
        }
        if let Some(strategy) = self.create_strategy_for_kind(DataType::Object(object)) {
            return strategy;
        }
        panic!("Could not find matching schema type for object: {object}")
    }

    /// Get the current active strategy for the schema, if not found create a new one.
    fn get_or_create_strategy_for_schema(&mut self, schema: &Value) -> &mut BasicSchemaStrategy {
        if let Some(idx) = self.get_strategy_for_kind(DataType::Schema(schema)) {
            return &mut self.active_strategies[idx];
        }
        if let Some(strategy) = self.create_strategy_for_kind(DataType::Schema(schema)) {
            return strategy;
        }
        panic!("Could not find matching schema type for schema: {schema}")
    }

    /// Get the strategy that matches the schema or object and return its index.
    fn get_strategy_for_kind(&self, schema_or_object: DataType) -> Option<usize> {
        self.active_strategies.iter().position(|strategy| {
            SchemaNode::strategy_does_match_schema_or_object(strategy, &schema_or_object)
        })
    }

    fn create_strategy_for_kind(
        &mut self,
        schema_or_object: DataType,
    ) -> Option<&mut BasicSchemaStrategy> {
        if let Some(mut strategy) =
            SchemaNode::create_strategy_for_schema_or_object(&schema_or_object)
        {
            if let Some(BasicSchemaStrategy::Typeless(typeless)) = self.active_strategies.last() {
                // if the last strategy is a typeless strategy, incorporate it into the newly created strategy
                SchemaNode::add_schema_or_object_to_strategy(
                    &mut strategy,
                    DataType::Schema(&typeless.to_schema()),
                );
                self.active_strategies.pop();
            }
            self.active_strategies.push(strategy);
            return Some(self.active_strategies.last_mut().unwrap());
        }
        // if no matching strategy found, create a typeless strategy and append to the active strategies
        // list if it's currently empty
        // ??: don't really understand the significance of typeless strategy yet
        else if let DataType::Schema(schema) = schema_or_object {
            if TypelessStrategy::match_schema(schema) {
                if self.active_strategies.is_empty() {
                    self.active_strategies
                        .push(BasicSchemaStrategy::Typeless(TypelessStrategy::new()));
                }
                let first_strategy = self.active_strategies.first_mut().unwrap();
                return Some(first_strategy);
            }
        }
        None
    }

    fn strategy_does_match_schema_or_object(
        strategy: &BasicSchemaStrategy,
        schema_or_object: &DataType,
    ) -> bool {
        match schema_or_object {
            DataType::Object(obj) => strategy.match_object(obj),
            DataType::Schema(schema) => strategy.match_schema(schema),
            _ => false,
        }
    }

    /// Create a strategy for a schema or object based on which strategy it matches.
    fn create_strategy_for_schema_or_object(
        schema_or_object: &DataType,
    ) -> Option<BasicSchemaStrategy> {
        match schema_or_object {
            DataType::Object(obj) => BasicSchemaStrategy::new_for_object(obj),
            DataType::Schema(schema) => BasicSchemaStrategy::new_for_schema(schema),
            _ => None,
        }
    }

    fn add_schema_or_object_to_strategy(
        strategy: &mut BasicSchemaStrategy,
        schema_or_object: DataType,
    ) {
        match schema_or_object {
            DataType::Object(obj) => strategy.add_object(obj),
            DataType::Schema(schema) => strategy.add_schema(schema),
            _ => (),
        }
    }
}
