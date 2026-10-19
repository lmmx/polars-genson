"""PROTOTYPE (measurement only, upper bound): prepare_json_bytes without serde validation.

Replaces the serde_json validation pass of each string (`validate_ndjson` / `validate_json`
in `prepare_json_bytes`) with `Ok(())`, leaving simd-json as the only parse. Inputs with
invalid JSON then fail later, with simd-json's error; the corpora are valid.

Usage: python proto_no_validate.py <checkout>
"""

import sys

p = sys.argv[1] + "/genson-core/src/schema.rs"
with open(p) as f:
    s = f.read()
for old in (
    "            validate_ndjson(json_str)\n",
    "        validate_json(json_str)\n",
):
    assert s.count(old) == 1, old
    s = s.replace(old, old.replace(old.strip(), "Ok::<(), serde_json::Error>(())"))
with open(p, "w") as f:
    f.write(s)
print("proto_no_validate applied")
