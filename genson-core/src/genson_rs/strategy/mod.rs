pub mod array;
pub mod base;
pub mod object;
pub mod scalar;

use serde_json::Value;

use array::{ListStrategy, TupleStrategy};
use base::{ScalarSchemaStrategy, SchemaStrategy};
use object::ObjectStrategy;
use scalar::{BooleanStrategy, NullStrategy, NumberStrategy, StringStrategy, TypelessStrategy};

use self::array::ListSchemaStrategy;

#[derive(Debug, PartialEq)]
pub enum BasicSchemaStrategy {
    Object(ObjectStrategy),
    List(ListStrategy),
    Tuple(TupleStrategy),
    Null(NullStrategy),
    Boolean(BooleanStrategy),
    Number(NumberStrategy),
    String(StringStrategy),
    Typeless(TypelessStrategy),
}

// TODO: the match check is repeated everywhere, maybe we can optimize this with a macro!

impl BasicSchemaStrategy {
    pub fn new_for_object(object: crate::genson_rs::JsonValue) -> Option<Self> {
        if ObjectStrategy::match_object(object) {
            Some(BasicSchemaStrategy::Object(ObjectStrategy::new()))
        } else if <ListStrategy as ListSchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::List(ListStrategy::new()))
        } else if <TupleStrategy as ListSchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::Tuple(TupleStrategy::new()))
        } else if <NullStrategy as SchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::Null(NullStrategy::new()))
        } else if <BooleanStrategy as SchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::Boolean(BooleanStrategy::new()))
        } else if <NumberStrategy as SchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::Number(NumberStrategy::new()))
        } else if <StringStrategy as SchemaStrategy>::match_object(object) {
            Some(BasicSchemaStrategy::String(StringStrategy::new()))
        } else {
            None
        }
    }

    pub fn new_for_schema(schema: &Value) -> Option<Self> {
        if ObjectStrategy::match_schema(schema) {
            Some(BasicSchemaStrategy::Object(ObjectStrategy::new()))
        } else if ListStrategy::match_schema(schema) {
            Some(BasicSchemaStrategy::List(ListStrategy::new()))
        } else if TupleStrategy::match_schema(schema) {
            Some(BasicSchemaStrategy::Tuple(TupleStrategy::new()))
        } else if <NullStrategy as SchemaStrategy>::match_schema(schema) {
            Some(BasicSchemaStrategy::Null(NullStrategy::new()))
        } else if <BooleanStrategy as SchemaStrategy>::match_schema(schema) {
            Some(BasicSchemaStrategy::Boolean(BooleanStrategy::new()))
        } else if <NumberStrategy as SchemaStrategy>::match_schema(schema) {
            Some(BasicSchemaStrategy::Number(NumberStrategy::new()))
        } else if <StringStrategy as SchemaStrategy>::match_schema(schema) {
            Some(BasicSchemaStrategy::String(StringStrategy::new()))
        } else {
            None
        }
    }

    pub fn same_kind(&self, other: &BasicSchemaStrategy) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// A new, empty strategy of the same kind.
    pub fn new_of_same_kind(&self) -> Self {
        match self {
            BasicSchemaStrategy::Object(_) => BasicSchemaStrategy::Object(ObjectStrategy::new()),
            BasicSchemaStrategy::List(_) => BasicSchemaStrategy::List(ListStrategy::new()),
            BasicSchemaStrategy::Tuple(_) => BasicSchemaStrategy::Tuple(TupleStrategy::new()),
            BasicSchemaStrategy::Null(_) => BasicSchemaStrategy::Null(NullStrategy::new()),
            BasicSchemaStrategy::Boolean(_) => BasicSchemaStrategy::Boolean(BooleanStrategy::new()),
            BasicSchemaStrategy::Number(_) => BasicSchemaStrategy::Number(NumberStrategy::new()),
            BasicSchemaStrategy::String(_) => BasicSchemaStrategy::String(StringStrategy::new()),
            BasicSchemaStrategy::Typeless(_) => {
                BasicSchemaStrategy::Typeless(TypelessStrategy::new())
            }
        }
    }

    /// The type, when this strategy's schema is nothing but `{"type": <type>}` (such schemas
    /// are collapsed into one type list by `SchemaNode::to_schema`).
    pub fn bare_type(&self) -> Option<&'static str> {
        let no_keywords = |keywords: &Value| keywords.as_object().is_some_and(|k| k.is_empty());
        match self {
            BasicSchemaStrategy::Object(strategy) => strategy.is_bare().then_some("object"),
            BasicSchemaStrategy::List(_)
            | BasicSchemaStrategy::Tuple(_)
            | BasicSchemaStrategy::Typeless(_) => None,
            BasicSchemaStrategy::Null(strategy) => {
                no_keywords(strategy.get_extra_keywords()).then_some("null")
            }
            BasicSchemaStrategy::Boolean(strategy) => {
                no_keywords(strategy.get_extra_keywords()).then_some("boolean")
            }
            BasicSchemaStrategy::Number(strategy) => {
                no_keywords(strategy.get_extra_keywords()).then(|| strategy.number_type())
            }
            BasicSchemaStrategy::String(strategy) => {
                no_keywords(strategy.get_extra_keywords()).then_some("string")
            }
        }
    }

    /// Merge `other`, of the same kind, into this strategy: the same as
    /// `self.add_schema(&other.to_schema())`. Objects and lists merge their child nodes
    /// directly; the other kinds hold no more than a few keywords, so take that route.
    pub fn absorb(&mut self, other: BasicSchemaStrategy) {
        match (self, other) {
            (BasicSchemaStrategy::Object(strategy), BasicSchemaStrategy::Object(other)) => {
                strategy.absorb(other)
            }
            (BasicSchemaStrategy::List(strategy), BasicSchemaStrategy::List(other)) => {
                strategy.absorb(other)
            }
            (strategy, other) => strategy.add_schema(&other.to_schema()),
        }
    }

    /// Feed `hasher` what identifies this strategy's schema (see `SchemaNode::hash_schema`).
    pub fn hash_schema<H: std::hash::Hasher>(&self, hasher: &mut H) {
        match self {
            BasicSchemaStrategy::Object(strategy) => {
                hasher.write_u8(b'O');
                strategy.hash_schema(hasher);
            }
            BasicSchemaStrategy::List(strategy) => {
                hasher.write_u8(b'L');
                strategy.items_node().hash_schema(hasher);
            }
            _ => {
                hasher.write_u8(b'S');
                crate::genson_rs::node::hash_value(&self.to_schema(), hasher);
            }
        }
    }

    pub fn to_schema(&self) -> Value {
        match self {
            BasicSchemaStrategy::Object(strategy) => strategy.to_schema(),
            BasicSchemaStrategy::List(strategy) => ListSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::Tuple(strategy) => ListSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::Null(strategy) => ScalarSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::Boolean(strategy) => ScalarSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::Number(strategy) => ScalarSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::String(strategy) => ScalarSchemaStrategy::to_schema(strategy),
            BasicSchemaStrategy::Typeless(strategy) => strategy.to_schema(),
        }
    }

    pub fn match_object(&self, object: crate::genson_rs::JsonValue) -> bool {
        match self {
            BasicSchemaStrategy::Object(_) => ObjectStrategy::match_object(object),
            BasicSchemaStrategy::List(_) => {
                <ListStrategy as ListSchemaStrategy>::match_object(object)
            }
            BasicSchemaStrategy::Tuple(_) => {
                <TupleStrategy as ListSchemaStrategy>::match_object(object)
            }
            BasicSchemaStrategy::Null(_) => <NullStrategy as SchemaStrategy>::match_object(object),
            BasicSchemaStrategy::Boolean(_) => {
                <BooleanStrategy as SchemaStrategy>::match_object(object)
            }
            BasicSchemaStrategy::Number(_) => {
                <NumberStrategy as SchemaStrategy>::match_object(object)
            }
            BasicSchemaStrategy::String(_) => {
                <StringStrategy as SchemaStrategy>::match_object(object)
            }
            BasicSchemaStrategy::Typeless(_) => TypelessStrategy::match_object(object),
        }
    }

    pub fn match_schema(&self, schema: &Value) -> bool {
        match self {
            BasicSchemaStrategy::Object(_) => ObjectStrategy::match_schema(schema),
            BasicSchemaStrategy::List(_) => ListStrategy::match_schema(schema),
            BasicSchemaStrategy::Tuple(_) => TupleStrategy::match_schema(schema),
            BasicSchemaStrategy::Null(_) => <NullStrategy as SchemaStrategy>::match_schema(schema),
            BasicSchemaStrategy::Boolean(_) => {
                <BooleanStrategy as SchemaStrategy>::match_schema(schema)
            }
            BasicSchemaStrategy::Number(_) => {
                <NumberStrategy as SchemaStrategy>::match_schema(schema)
            }
            BasicSchemaStrategy::String(_) => {
                <StringStrategy as SchemaStrategy>::match_schema(schema)
            }
            BasicSchemaStrategy::Typeless(_) => TypelessStrategy::match_schema(schema),
        }
    }

    pub fn add_schema(&mut self, schema: &Value) {
        match self {
            BasicSchemaStrategy::Object(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::List(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::Tuple(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::Null(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::Boolean(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::Number(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::String(strategy) => strategy.add_schema(schema),
            BasicSchemaStrategy::Typeless(strategy) => strategy.add_schema(schema),
        }
    }

    /// Add multiple schemas at once - delegates to strategy-specific implementation
    pub fn add_schemas(&mut self, schemas: &[&Value]) {
        match self {
            BasicSchemaStrategy::Object(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::List(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::Tuple(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::Boolean(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::Number(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::String(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::Null(strategy) => strategy.add_schemas(schemas),
            BasicSchemaStrategy::Typeless(strategy) => strategy.add_schemas(schemas),
        }
    }

    pub fn add_object(&mut self, object: crate::genson_rs::JsonValue) {
        match self {
            BasicSchemaStrategy::Object(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::List(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::Tuple(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::Null(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::Boolean(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::Number(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::String(strategy) => strategy.add_object(object),
            BasicSchemaStrategy::Typeless(strategy) => strategy.add_object(object),
        }
    }
}
