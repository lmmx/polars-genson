"""Count the data objects that violate a schema's `required` lists.

Walks each row alongside the schema (properties, additionalProperties for maps, items,
the object branch of a nullable type) and, at every object whose schema has `required`,
counts the objects missing a required key. A schema inferred from the data should have
no violations.

Usage: python check_required.py <file.ndjson> <schema.json> [wrap field]
"""

import json
import sys
from collections import Counter
from pathlib import Path


def check(v, s, path, objs, bad, examples):
    if not isinstance(s, dict):
        return
    for alt in s.get("anyOf") or []:
        if isinstance(v, dict) and alt.get("type") in ("object", ["null", "object"]):
            s = alt
    if isinstance(v, dict):
        req = s.get("required")
        if req:
            objs[path] += 1
            missing = [k for k in req if k not in v]
            if missing:
                bad[path] += 1
                examples.setdefault(path, missing)
        props = s.get("properties") or {}
        ap = s.get("additionalProperties")
        for k, sub in v.items():
            if k in props:
                check(sub, props[k], f"{path}/{k}", objs, bad, examples)
            elif isinstance(ap, dict):
                check(sub, ap, f"{path}/*", objs, bad, examples)
    elif isinstance(v, list) and isinstance(s.get("items"), dict):
        for x in v:
            check(x, s["items"], f"{path}/[]", objs, bad, examples)


def main(corpus, schema_path, wrap=None):
    schema = json.loads(Path(schema_path).read_text())
    objs, bad, examples = Counter(), Counter(), {}
    with open(corpus) as f:
        for line in f:
            row = json.loads(line)
            check({wrap: row} if wrap else row, schema, "", objs, bad, examples)
    total = sum(objs.values())
    print(
        f"{schema_path.rsplit('/', 1)[-1]}: {total:,} objects under a required list, {sum(bad.values()):,} violate it, at {len(bad)} of {len(objs)} paths"
    )
    for path, n in bad.most_common(6):
        print(f"  {n:,}/{objs[path]:,} at {path} missing e.g. {examples[path][:5]}")


if __name__ == "__main__":
    main(*sys.argv[1:])
