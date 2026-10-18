"""PROTOTYPE (measurement only): merging a bare scalar strategy without its schema value.

`BasicSchemaStrategy::absorb` merges every kind but objects and lists through
`strategy.add_schema(&other.to_schema())`. For a scalar with no keywords that is a no-op,
except that a `number` turns an `integer` into `number`; the prototype does just that.

Usage: python proto_merge_scalar.py <checkout>
"""

import sys

p = sys.argv[1] + "/genson-core/src/genson_rs/strategy/mod.rs"
with open(p) as f:
    s = f.read()
old = "            (strategy, other) => strategy.add_schema(&other.to_schema()),\n"
new = """            // PROTOTYPE: a bare scalar adds nothing to one of its kind but number over integer
            (strategy, other) if other.bare_type().is_some() && strategy.same_kind(&other) => {
                if other.bare_type() == Some("number") {
                    strategy.add_schema(&serde_json::json!({"type": "number"}));
                }
            }
            (strategy, other) => strategy.add_schema(&other.to_schema()),
"""
assert s.count(old) == 1
s = s.replace(old, new)
with open(p, "w") as f:
    f.write(s)
print("proto_merge_scalar applied")
