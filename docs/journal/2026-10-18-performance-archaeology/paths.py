"""Size of the record interpretation versus the map interpretation of a corpus.

Counts node positions (paths of keys, with `[]` for list items) per row and across the
corpus, as the builder holds them before map inference, and again with the keys of the
given map paths collapsed to `*` (as the WIKIDATA=1 schema treats them). Also counts how
many rows' trees each union path occurs in.

Usage: python paths.py <file.ndjson> <final schema with maps (json)>
"""

import json
import sys
from collections import Counter
from pathlib import Path


def map_paths(s, path, out):
    if not isinstance(s, dict):
        return
    if isinstance(s.get("additionalProperties"), dict):
        out.add(path)
        map_paths(s["additionalProperties"], (*path, "*"), out)
    for k, v in (s.get("properties") or {}).items():
        map_paths(v, (*path, k), out)
    if isinstance(s.get("items"), dict):
        map_paths(s["items"], (*path, "[]"), out)
    for v in s.get("anyOf") or []:
        map_paths(v, path, out)


def walk(v, path, out, collapsed, maps):
    out.add(path)
    if isinstance(v, dict):
        for k, sub in v.items():
            key = "*" if collapsed and path in maps else k
            walk(sub, (*path, key), out, collapsed, maps)
    elif isinstance(v, list):
        for x in v:
            walk(x, (*path, "[]"), out, collapsed, maps)


def main(corpus, schema_path):
    maps = set()
    map_paths(json.loads(Path(schema_path).read_text()), (), maps)
    union, union_c = Counter(), set()
    per_row = per_row_c = rows = 0
    with open(corpus) as f:
        for line in f:
            doc = {"claims": json.loads(line)}
            a, b = set(), set()
            walk(doc, (), a, False, maps)
            walk(doc, (), b, True, maps)
            per_row += len(a)
            per_row_c += len(b)
            union.update(a)
            union_c |= b
            rows += 1
    once = sum(1 for c in union.values() if c == 1)
    print(f"rows {rows}")
    print(
        f"record interpretation: {per_row:,} node positions summed over rows ({per_row / rows:,.0f} per row), {len(union):,} in the union"
    )
    print(
        f"  union paths seen in one row only: {once:,} ({once / len(union):.0%}); mean rows per union path {sum(union.values()) / len(union):.1f}"
    )
    print(
        f"map interpretation ({len(maps)} map paths collapsed): {per_row_c:,} summed ({per_row_c / rows:,.0f} per row), {len(union_c):,} in the union"
    )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
