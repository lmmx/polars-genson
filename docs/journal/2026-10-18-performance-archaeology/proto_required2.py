"""PROTOTYPE (measurement only): proto_required.py keeping master's field order.

As proto_required.py, a field is required in the unified record only when every input
requires it, and nullable otherwise; but the fields keep master's order (those in the
`properties` of every input first, then the rest), so that only `required` and
nullability change in the output, not the order of fields.

Usage: python proto_required2.py <checkout>
"""

import sys

p = sys.argv[1] + "/genson-core/src/schema/map_inference/unification.rs"
with open(p) as f:
    s = f.read()


def edit(old, new):
    global s
    assert s.count(old) == 1, old[:80]
    s = s.replace(old, new)


edit(
    """    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut unified_anyofs""",
    """    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    // PROTOTYPE: how many inputs require each field
    let mut required_in: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for &schema in schemas {
        if let Some(Value::Array(req)) = extract_field_from_nullable_schema(schema, "required") {
            for name in req.iter().filter_map(Value::as_str) {
                *required_in.entry(name).or_default() += 1;
            }
        }
    }
    let required_by_all = |name: &str| required_in.get(name).copied() == Some(schemas.len());
    let mut unified_anyofs""",
)
edit(
    """    let required: Vec<&str> = in_all.iter().map(|(name, _, _)| *name).collect();""",
    """    let required: Vec<&str> = in_all
        .iter()
        .map(|(name, _, _)| *name)
        .filter(|name| required_by_all(name))
        .collect();""",
)
edit(
    """    for (name, _, field_schema) in in_all {
        debug_verbose!(
            config,
            "Field `{name}` present in all schemas → keeping non-nullable"
        );
        properties.insert(name.to_string(), field_schema);
    }""",
    """    for (name, _, field_schema) in in_all {
        debug_verbose!(
            config,
            "Field `{name}` present in all schemas → keeping non-nullable"
        );
        let field_schema = if required_by_all(name) {
            field_schema
        } else {
            make_nullable(field_schema)
        };
        properties.insert(name.to_string(), field_schema);
    }""",
)
with open(p, "w") as f:
    f.write(s)
print("proto_required2 applied")
