"""Write rows of Wikidata-like claims, as NDJSON (for `genson-core/examples/bench_claims.rs`)
or, if the output ends in `.parquet`, as a Parquet file of one string column, `claims`
(for `workloads.py`).

Each row is a random half of the properties of one entity's claims, each property keeping
a random number of its statements. The entities are the one-line fixtures in
`genson-cli/tests/data/claims/x1818_L*.jsonl` and the rows of `data/claims.parquet`.

Usage: python claims_rows.py <rows> <out.ndjson|out.parquet> [seed]
"""

import json
import random
import sys
from pathlib import Path

import polars as pl

HERE = Path(__file__).parent
FIXTURES = HERE.parent / "genson-cli" / "tests" / "data" / "claims"


def entities() -> list[dict]:
    pool = [json.loads(p.read_text()) for p in sorted(FIXTURES.glob("x1818_L*.jsonl"))]
    claims = pl.read_parquet(HERE / "data" / "claims.parquet")["claims"]
    return pool + [json.loads(c) for c in claims.drop_nulls()]


def main(rows: int, out: Path, seed: int = 0) -> None:
    rng = random.Random(seed)
    pool = entities()
    lines = []
    for _ in range(rows):
        entity = rng.choice(pool)
        keys = [k for k in entity if rng.random() < 0.5]
        row = {k: entity[k][: rng.randint(1, len(entity[k]))] for k in keys}
        lines.append(json.dumps(row))
    if out.suffix == ".parquet":
        pl.DataFrame({"claims": lines}).write_parquet(out)
    else:
        out.write_text("".join(line + "\n" for line in lines))


if __name__ == "__main__":
    main(int(sys.argv[1]), Path(sys.argv[2]), *map(int, sys.argv[3:]))
