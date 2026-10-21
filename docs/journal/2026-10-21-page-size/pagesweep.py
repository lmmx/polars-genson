"""Page-limit sweep with the duel binary (scratch build reading GENSON_PAGE_ROWS and
GENSON_PAGE_BYTES): per table, ROUNDS rounds of one fresh process per variant and one of
Polars, order rotated each round, after one discarded warm-up each. Runs go to
pagesweep.jsonl. Usage: python pagesweep.py <duel binary> <table> [<table> ...]
"""

import json
import os
import statistics
import subprocess
import sys
from pathlib import Path

import pyarrow.parquet as pq

HERE = Path(__file__).parent
ROUNDS = int(os.environ.get("DUEL_ROUNDS", 5))
UNLIMITED = str(2**62)
VARIANTS = {
    "genson 20k rows/1MiB": ("genson", {}),
    "genson no rows/1MiB": ("genson", {"GENSON_PAGE_ROWS": UNLIMITED}),
    "genson no rows/2MiB": (
        "genson",
        {"GENSON_PAGE_ROWS": UNLIMITED, "GENSON_PAGE_BYTES": str(2 << 20)},
    ),
    "polars": ("polars", {}),
}
binary, tables = sys.argv[1], sys.argv[2:]
out = HERE / "out"
out.mkdir(exist_ok=True)


def run(name, table):
    writer, env = VARIANTS[name]
    dst = out / f"{table}.page.parquet"
    p = subprocess.run(
        [binary, writer, str(HERE / "data" / f"{table}.parquet"), str(dst)],
        capture_output=True,
        text=True,
        check=True,
        env=dict(os.environ, **env),
    )
    r = json.loads(p.stdout.strip().splitlines()[-1])
    r.update(
        table=table,
        name=name,
        bytes=dst.stat().st_size,
        row_groups=pq.ParquetFile(dst).metadata.num_row_groups,
    )
    dst.unlink()
    return r


def fmt(xs):
    return f"{statistics.median(xs):.3f} ({min(xs):.3f}-{max(xs):.3f})"


names = list(VARIANTS)
with open(HERE / "pagesweep.jsonl", "a") as log:
    for table in tables:
        for n in names:
            run(n, table)
        runs = {n: [] for n in names}
        for r in range(ROUNDS):
            k = r % len(names)
            for n in names[k:] + names[:k]:
                res = run(n, table)
                runs[n].append(res)
                log.write(json.dumps(res) + "\n")
                log.flush()
        for n in names:
            rs = runs[n]
            print(
                f"{table:13} {n:22} user {fmt([x['user'] for x in rs])}  wall {fmt([x['wall'] for x in rs])}  "
                f"{rs[0]['bytes'] / 1e6:8.2f} MB  {rs[0]['row_groups']} rg",
                flush=True,
            )
