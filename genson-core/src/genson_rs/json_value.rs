//! Thin adapter layer over sonic_rs::Value to match the trait interface
//! we were using from simd_json

use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};

/// Extension trait to match simd_json's API patterns
pub trait ValueExt {
    fn is_object(&self) -> bool;
    fn is_array(&self) -> bool;
    fn is_str(&self) -> bool;
    fn is_number(&self) -> bool;
    fn is_bool(&self) -> bool;
    fn is_null(&self) -> bool;
    fn is_f64(&self) -> bool;
    fn is_i64(&self) -> bool;
    fn is_u64(&self) -> bool;
}

impl ValueExt for Value {
    fn is_object(&self) -> bool {
        JsonValueTrait::is_object(self)
    }
    fn is_array(&self) -> bool {
        JsonValueTrait::is_array(self)
    }
    fn is_str(&self) -> bool {
        JsonValueTrait::is_str(self)
    }
    fn is_number(&self) -> bool {
        JsonValueTrait::is_number(self)
    }
    fn is_bool(&self) -> bool {
        JsonValueTrait::is_boolean(self)
    }
    fn is_null(&self) -> bool {
        JsonValueTrait::is_null(self)
    }
    fn is_f64(&self) -> bool {
        JsonValueTrait::is_f64(self)
    }
    fn is_i64(&self) -> bool {
        JsonValueTrait::is_i64(self)
    }
    fn is_u64(&self) -> bool {
        JsonValueTrait::is_u64(self)
    }
}

/// Object iteration helper
pub fn iter_object(v: &Value) -> impl Iterator<Item = (&str, &Value)> {
    v.as_object()
        .map(|obj: &Object| obj.iter())
        .into_iter()
        .flatten()
}

/// Array iteration helper
pub fn iter_array(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}
