"""PROTOTYPE (measurement only): the record branch of normalise_inner moves fields out.

At master each field's input value is `m.get(name).cloned()` from the record's input
object (`field_value`), so a value nested d records deep is copied d times. The record's
input is owned (`normalise_inner` takes `value: Value`), so with no `prune` names (which
can hand the whole input back as pruned) each field can be taken out of the map instead.

Usage: python proto_norm_move.py <checkout>
"""

import sys

p = sys.argv[1] + "/genson-core/src/normalise.rs"
with open(p) as f:
    s = f.read()


def edit(old, new):
    global s
    assert s.count(old) == 1, old[:80]
    s = s.replace(old, new)


edit(
    """/// Whether a record's input value holds a non-null value for its field `name`.""",
    """/// PROTOTYPE: `field_value`, taking the field out of an object instead of cloning it.
fn take_field_value(value: &mut Value, name: &str, has_int_field: bool) -> Value {
    match value {
        Value::Object(m) => m.remove(name).unwrap_or(Value::Null),
        scalar if promoted_scalar_matches(scalar, name, has_int_field) => scalar.clone(),
        _ => Value::Null,
    }
}

/// Whether a record's input value holds a non-null value for its field `name`.""",
)
edit(
    """                let start = pruned.len();
                for f in fields {""",
    """                let start = pruned.len();
                let mut value = value;
                for f in fields {""",
)
edit(
    """                        let val = field_value(&value, name, has_int_field);""",
    """                        let val = if prune.is_empty() {
                            take_field_value(&mut value, name, has_int_field)
                        } else {
                            field_value(&value, name, has_int_field)
                        };""",
)
with open(p, "w") as f:
    f.write(s)
print("proto_norm_move applied")
