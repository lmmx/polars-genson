//! Building `serde_json` values without copying their parts.

use serde_json::Value;

/// A JSON object of `fields`, in order, each value moved in: `json!` writes each value it
/// is given through `serde_json::to_value`, which copies a whole `Value` by serialising it.
pub(crate) fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}
