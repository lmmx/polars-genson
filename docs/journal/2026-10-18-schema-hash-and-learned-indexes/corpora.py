"""Write the two NDJSON corpora used beside rows200 in this entry.

- labels.ndjson: the `labels` column of bench/data/labels.parquet, one row per line.
- narrow200k.ndjson: 200,000 synthetic narrow rows (six fields, a nested record and a list),
  seed 0, where most rows share a handful of shapes.

rows200 comes from `bench/claims_rows.py 200 rows200.ndjson` (seed 0).

Usage: uv run --no-project --with polars python corpora.py \
    bench/data/labels.parquet labels.ndjson narrow200k.ndjson
"""

import json
import random
import sys

import polars as pl

src, out_labels, out_narrow = sys.argv[1:4]
df = pl.read_parquet(src)
with open(out_labels, "w") as f:
    f.writelines(s.replace("\n", " ") + "\n" for s in df["labels"].drop_nulls())
rng = random.Random(0)
with open(out_narrow, "w") as f:
    for i in range(200_000):
        row = {
            "id": i,
            "name": f"user{i}",
            "active": rng.random() < 0.5,
            "score": rng.random() if rng.random() < 0.3 else rng.randint(0, 9),
            "tags": [rng.choice("abcdef") for _ in range(rng.randint(0, 3))],
            "meta": {"source": rng.choice(["web", "app"]), "v": rng.randint(1, 3)},
        }
        if rng.random() < 0.2:
            row["meta"]["extra"] = None
        f.write(json.dumps(row) + "\n")
