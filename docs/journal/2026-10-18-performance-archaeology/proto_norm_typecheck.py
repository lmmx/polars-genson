"""PROTOTYPE (measurement only): normalise_inner's schema-type checks without allocating.

`obj.get("type") == Some(&Value::String("record".into()))` builds a `String` (and a
`Value`) for every comparison, and normalise_inner makes up to three such comparisons per
input value it visits; the array and map branches also build a default `Value` each call.
The prototype compares with `as_str` instead.

Usage: python proto_norm_typecheck.py <checkout>
"""

import re
import sys

p = sys.argv[1] + "/genson-core/src/normalise.rs"
with open(p) as f:
    s = f.read()
s, n = re.subn(
    r'obj\.get\("type"\) == Some\(&Value::String\("(\w+)"\.into\(\)\)\)',
    r'obj.get("type").and_then(Value::as_str) == Some("\1")',
    s,
)
assert n >= 4, n
with open(p, "w") as f:
    f.write(s)
print(f"proto_norm_typecheck applied ({n} comparisons)")
