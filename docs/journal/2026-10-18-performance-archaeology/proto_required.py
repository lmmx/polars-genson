"""PROTOTYPE (measurement only): unify_record_schemas takes `required` from its inputs.

A field is required in the unified record only when every input schema requires it (an
input without a `required` array requires nothing), and every field not required that
way is made nullable. At master a field is required, and kept non-nullable, when it is
in the `properties` of every input, whatever the inputs' own `required`: re-unifying an
already unified schema then turns its optional fields back into required ones.

Usage: python proto_required.py <checkout>
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
    let mut unified_anyofs""",
)
edit(
    """        .map(|(&name, field_schema)| (name, counts[name], field_schema))
        .partition(|(_, count, _)| *count == schemas.len());""",
    """        .map(|(&name, field_schema)| (name, counts[name], field_schema))
        .partition(|(name, _, _)| required_in.get(name).copied() == Some(schemas.len()));""",
)
with open(p, "w") as f:
    f.write(s)
print("proto_required applied")
