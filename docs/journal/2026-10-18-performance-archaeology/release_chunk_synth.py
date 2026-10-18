"""A release-format chunk of a main-set chunk's shape, made offline from entity JSON already
fetched (`release_chunk.py`'s `<out>/entities/`), through wikidata-pq's own reshaping
(`wikidata.dump.write_chunk`, as `split-dump` writes a chunk: Polars, zstd level 9).

The fetched entities are well-known items, hundreds of kB each; a main-set chunk of
release 20260928 has a median of 7,128 entities in about 3.7 MB (wikidata-pq
docs/journal/2026-10-03-release-20260928-run.md). Each entity made here takes one fetched
entity as its template and keeps a random few of its properties (a few statements each),
labels, descriptions, aliases and sitelinks, under a new id (statement ids follow it), so
every snak, qualifier and reference is one Wikidata wrote, in proportions closer to a
typical chunk's. The draw is seeded.

Usage: python release_chunk_synth.py <entities dir> <out dir> <rows> [seed]
"""

import json
import random
import sys
from pathlib import Path

import orjson
from wikidata import dump

src, out, rows = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
rng = random.Random(int(sys.argv[4]) if len(sys.argv) > 4 else 0)
templates = []
for path in sorted(src.glob("Q*.json")):
    entity = orjson.loads(path.read_bytes())["entities"]
    templates.append(next(iter(entity.values())))


def few(mean: float) -> int:
    """A count from a geometric distribution of the given mean (at least 1)."""
    n = 1
    while rng.random() > 1 / mean:
        n += 1
    return n


def subset(mapping: object, mean: float) -> dict:
    if not isinstance(mapping, dict) or not mapping:
        return {}
    keys = rng.sample(sorted(mapping), min(len(mapping), few(mean)))
    return {k: mapping[k] for k in keys}


def entity(n: int) -> dict:
    template = rng.choice(templates)
    qid = f"Q{1_000_000 + n}"
    e = {k: v for k, v in template.items() if k not in dump.ENTITY_PARTS}
    e["id"] = qid
    e["title"] = qid
    e["labels"] = subset(template.get("labels"), 6)
    e["descriptions"] = subset(template.get("descriptions"), 4)
    e["aliases"] = subset(template.get("aliases"), 1.5) if rng.random() < 0.3 else {}
    e["sitelinks"] = subset(template.get("sitelinks"), 2) if rng.random() < 0.5 else {}
    claims = {}
    for prop, statements in subset(template.get("claims"), 4).items():
        kept = rng.sample(statements, min(len(statements), few(1.4)))
        claims[prop] = [
            {**s, "id": f"{qid}${s['id'].split('$', 1)[-1]}"} if "id" in s else s for s in kept
        ]
    e["claims"] = claims
    return e


lines = [orjson.dumps(entity(n)) for n in range(rows)]
entry = dump.write_chunk(0, lines, out)
print(json.dumps({k: entry[k] for k in ("file", "rows", "bytes", "first", "last")}))
