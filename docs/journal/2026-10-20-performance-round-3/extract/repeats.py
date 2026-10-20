"""How much of the claims column's text is in the fields extraction removes, and how much
of that repeats a (field, key) already seen with the same text. Approximate: values
re-serialised by json.dumps(separators) and keys found by walking the parsed rows."""
import json, sys
import polars as pl
SPEC = {"labels": "id", "property-labels": "property", "unit-labels": "unit"}
col = pl.read_parquet(sys.argv[1], columns=["claims"])["claims"]
total = removed = repeat = 0
seen = {}
def walk(v):
    global removed, repeat
    if isinstance(v, dict):
        for f, k in SPEC.items():
            if f in v and v.get(k) is not None:
                t = json.dumps(v[f], separators=(",", ":"), ensure_ascii=False)
                removed += len(t.encode())
                key = (f, json.dumps(v[k]))
                if seen.get(key) == t:
                    repeat += len(t.encode())
                seen.setdefault(key, t)
        for f, x in v.items():
            if f not in SPEC:
                walk(x)
    elif isinstance(v, list):
        for x in v:
            walk(x)
for s in col:
    if s is None:
        continue
    total += len(s.encode())
    walk(json.loads(s))
print(f"{sys.argv[1]}: rows {len(col)}, text {total/1e6:.1f} MB, removed {removed/1e6:.1f} MB ({removed/total:.0%}), repeats {repeat/1e6:.1f} MB ({repeat/max(removed,1):.0%} of removed), distinct entries {len(seen)}")
