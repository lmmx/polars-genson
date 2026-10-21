"""Dictionary threshold sweep with the duel binary (scratch build of #290 whose threshold
is read from GENSON_DICT_THRESHOLD).

For each table: the per-leaf decisions each threshold makes (from GENSON_DICT_LOG), the
thresholds grouped by identical decisions (identical files, checked with cmp), then
ROUNDS rounds of one fresh process per group and one of Polars, order rotated each round,
after one discarded warm-up each. Every run goes to sweep.jsonl.

Usage: python sweep.py <duel binary> <table> [<table> ...]   (env DUEL_ROUNDS, default 5)
"""

import filecmp
import json
import os
import statistics
import subprocess
import sys
from pathlib import Path

import pyarrow.parquet as pq

HERE = Path(__file__).parent
ROUNDS = int(os.environ.get("DUEL_ROUNDS", 5))
THRESHOLDS = [0.0, 0.25, 0.5, 0.75, 0.9, 1.0]
binary, tables = sys.argv[1], sys.argv[2:]
out = HERE / "out"
out.mkdir(exist_ok=True)


def run(writer, table, threshold=None, keep=False):
    env = dict(os.environ, GENSON_DICT_LOG="1")
    if threshold is not None:
        env["GENSON_DICT_THRESHOLD"] = str(threshold)
    name = writer if threshold is None else f"genson@{threshold}"
    dst = out / f"{table}.{name}.parquet"
    p = subprocess.run(
        [binary, writer, str(HERE / "data" / f"{table}.parquet"), str(dst)],
        capture_output=True,
        text=True,
        check=True,
        env=env,
    )
    r = json.loads(p.stdout.strip().splitlines()[-1])
    plain = tuple(
        line.split()[2]
        for line in p.stderr.splitlines()
        if line.startswith("DICT leaf") and line.endswith("plain=true")
    )
    sample = [
        float(line.split()[-1])
        for line in p.stderr.splitlines()
        if line.startswith("DICT sample_s")
    ]
    r.update(
        table=table,
        name=name,
        bytes=dst.stat().st_size,
        row_groups=pq.ParquetFile(dst).metadata.num_row_groups,
        sample_s=sample[0] if sample else None,
        plain=len(plain),
    )
    if not keep:
        dst.unlink()
    return r, plain, dst


def fmt(xs):
    return f"{statistics.median(xs):.3f} ({min(xs):.3f}-{max(xs):.3f})"


with open(HERE / "sweep.jsonl", "a") as log:
    for table in tables:
        # Group thresholds by their decisions; keep one file per group for cmp
        groups = {}
        for t in THRESHOLDS:
            _, plain, dst = run("genson", table, t, keep=True)
            groups.setdefault(plain, []).append((t, dst))
        reps = []
        for plain, members in groups.items():
            first = members[0][1]
            for _, other in members[1:]:
                assert filecmp.cmp(first, other, shallow=False), (table, members)
            for _, f in members:
                f.unlink()
            reps.append(([t for t, _ in members], len(plain)))
        variants = [("genson", ts[0]) for ts, _ in reps] + [("polars", None)]
        for w, t in variants:
            run(w, table, t)  # warm-up
        runs = {v: [] for v in variants}
        for r in range(ROUNDS):
            k = r % len(variants)
            for v in variants[k:] + variants[:k]:
                res, _, _ = run(v[0], table, v[1])
                runs[v].append(res)
                log.write(json.dumps(res) + "\n")
                log.flush()
        for (w, t), (ts, nplain) in zip(variants, reps + [(None, None)]):
            rs = runs[(w, t)]
            label = (
                "polars (1Mi rg)"
                if w == "polars"
                else "genson " + "/".join(f"{x:g}" for x in ts)
            )
            extra = (
                ""
                if w == "polars"
                else f"  plain {nplain:2} leaves  sample {statistics.median([x['sample_s'] for x in rs]) * 1e3:6.2f} ms"
            )
            print(
                f"{table:13} {label:26} user {fmt([x['user'] for x in rs])}  wall {fmt([x['wall'] for x in rs])}  "
                f"{rs[0]['bytes'] / 1e6:8.2f} MB  {rs[0]['row_groups']} rg{extra}",
                flush=True,
            )
