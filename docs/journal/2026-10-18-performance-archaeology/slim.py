"""Remove the label maps that wikidata-pq extracts before inference (extract_invariants).

Drops every `labels`, `property-labels` and `unit-labels` key, at any depth, from each row:
the fields `extract_invariants={"labels": "id", "property-labels": "property",
"unit-labels": "unit"}` moves to the lookup table (2026-09-26-extract-lookup.md).

Usage: python slim.py <in.ndjson> <out.ndjson>
"""

import json
import sys

DROP = {"labels", "property-labels", "unit-labels"}


def slim(v):
    if isinstance(v, dict):
        return {k: slim(x) for k, x in v.items() if k not in DROP}
    if isinstance(v, list):
        return [slim(x) for x in v]
    return v


with open(sys.argv[1]) as f, open(sys.argv[2], "w") as g:
    g.writelines(json.dumps(slim(json.loads(line))) + "\n" for line in f)
