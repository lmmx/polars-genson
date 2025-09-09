#[cfg(test)]
mod union_ordering {
    use genson_core::schema::reorder_unions;
    use serde_json::json;

    /// Basic sanity: null + other type keeps null first.
    #[test]
    fn test_union_null_and_string() {
        let mut schema = json!({"type":["null","string"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["null", "string"]));
    }

    /// Boolean should come before string.
    #[test]
    fn test_union_bool_string() {
        let mut schema = json!({"type":["string","boolean"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["boolean", "string"]));
    }

    /// Integer precedes number/float/double.
    #[test]
    fn test_union_int_number() {
        let mut schema = json!({"type":["number","integer"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["integer", "number"]));
    }

    /// Enum comes before string in canonical order.
    #[test]
    fn test_union_enum_string() {
        let mut schema = json!({"type":["string","enum"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["enum", "string"]));
    }

    /// Fixed comes before bytes.
    #[test]
    fn test_union_fixed_bytes() {
        let mut schema = json!({"type":["bytes","fixed"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["fixed", "bytes"]));
    }

    /// Map precedes array and object.
    #[test]
    fn test_union_map_array_object() {
        let mut schema = json!({"type":["object","map","array"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["map", "array", "object"]));
    }

    /// Unknown type string (e.g. "date") sorts after known types.
    #[test]
    fn test_union_with_unknown_string() {
        let mut schema = json!({"type":["string","date","integer"]});
        reorder_unions(&mut schema);
        assert_eq!(schema["type"], json!(["integer", "string", "date"]));
    }

    /// Inline object schemas sort to the start.
    #[test]
    fn test_union_with_inline_schema() {
        let mut schema = json!({"type":["string", {"type":"array","items":"string"}]});
        reorder_unions(&mut schema);
        let arr = schema["type"].as_array().unwrap();
        assert!(arr[0].is_object());
        assert_eq!(arr[1], json!("string"));
    }

    /// Stress test: full mix of core types.
    #[test]
    fn test_union_full_mix() {
        let mut schema = json!({"type":[
            "object","boolean","integer","null","array","string","number","map","bytes","fixed", "enum"
        ]});
        reorder_unions(&mut schema);
        let got = schema["type"].as_array().unwrap();
        let binding = json!([
            "null", "map", "array", "object", "boolean", "integer", "number", "enum", "string",
            "fixed", "bytes"
        ]);
        let expected = binding.as_array().unwrap();
        assert_eq!(got, &expected[..got.len()]);
    }
}
