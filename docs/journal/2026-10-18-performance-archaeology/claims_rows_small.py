"""Wikidata-like claims rows the size of an ordinary chunk's (tens of KB), as a chunk file.

As bench/claims_rows.py, from the same entities (genson-cli's x1818 fixtures and
bench/data/claims.parquet), but each row keeps each property with probability `share`
(default 0.05) instead of a half, so rows are about the size of a real chunk's entities
(chunk_1237: 625 MB of claims over 10,000 entities) rather than of the fixtures' large
ones. Writes `id` (Q0, Q1, ...) and `claims` (JSON) to a Parquet file.

Usage: python claims_rows_small.py <rows> <out.parquet> [share] [seed]
"""

import json
import random
import sys
from pathlib import Path

import polars as pl

REPO = Path(__file__).resolve().parents[3]
FIXTURES = REPO / "genson-cli" / "tests" / "data" / "claims"


def entities() -> list[dict]:
    pool = [json.loads(p.read_text()) for p in sorted(FIXTURES.glob("x1818_L*.jsonl"))]
    claims = pl.read_parquet(REPO / "bench" / "data" / "claims.parquet")["claims"]
    return pool + [json.loads(c) for c in claims.drop_nulls()]


def main(rows: int, out: Path, share: float = 0.05, seed: int = 0) -> None:
    rng = random.Random(seed)
    pool = entities()
    data = []
    for _ in range(rows):
        entity = rng.choice(pool)
        keys = [k for k in entity if rng.random() < share] or [rng.choice(list(entity))]
        row = {k: entity[k][: rng.randint(1, len(entity[k]))] for k in keys}
        data.append(json.dumps(row))
    ids = [f"Q{i}" for i in range(rows)]
    pl.DataFrame({"id": ids, "claims": data}).write_parquet(out)


if __name__ == "__main__":
    args = sys.argv[1:]
    main(int(args[0]), Path(args[1]), *(float(args[2]),) if len(args) > 2 else (), *map(int, args[3:]))
