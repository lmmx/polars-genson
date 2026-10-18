"""End-to-end timing of wikidata-pq's `process()` on one chunk: every table it writes
(labels, descriptions, aliases, links, claims and their label lookup), its id checks and
its claims schema check, as the pipeline runs it per chunk.

Each run is a fresh process in a fresh directory laid out as wikidata-pq's local mirror
(`data/huggingface_hub/<repo>/data/chunk_0.parquet`, a copy: `process()` deletes its
sources) or, with `WIKIDATA_RELEASE` set, as a release's split chunks
(`data/chunk_0.parquet`, with the release's claims schema and its `entity` table), with
the working directory there (its quarantine directory is relative). Prints
the wall and CPU time of the call (user + system, all threads), the peak RSS, and the wall
time of each step, by wrapping the functions `process()` calls, and the `[profile]` stage
lines polars-genson prints (summed by name; wikidata-pq passes `profile=True` for claims). With `MAP_ENCODING=kv`,
`normalise_from_parquet` and `avro_to_polars_schema` get `map_encoding="kv"`
(polars-genson >= 1.0 defaults to maps; wikidata-pq's tables are key/value lists).
Nothing is fetched: `process()` reads the mirror only. With `KEEP=dir`, each run's tables
(and the quarantine directory) are copied to `dir/<label>/<run>/` for `compare_tables.py`.

Usage: python bench_process.py <python> <chunk.parquet> <runs> [label]
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ONE_RUN = r"""
import functools, json, os, resource, sys, time
from pathlib import Path
import polars as pl, polars_genson
from wikidata import process
from wikidata.config import REPO_ID
if os.environ.get("MAP_ENCODING") == "kv":
    process.normalise_from_parquet = functools.partial(
        polars_genson.normalise_from_parquet, map_encoding="kv")
    process.avro_to_polars_schema = functools.partial(
        polars_genson.avro_to_polars_schema, map_encoding="kv")
steps = {}
def timed(name, fn):
    def run(*a, **k):
        t = time.perf_counter()
        try:
            return fn(*a, **k)
        finally:
            key = {
                "normalise_map_direct": lambda: f"map:{k.get('key')}",
                # By the table written, from its destination's directory
                "atomic_sink_parquet": lambda: f"sink:{Path(a[1]).parent.name}",
            }.get(name, lambda: name)()
            steps[key] = steps.get(key, 0) + time.perf_counter() - t
    return run
for name in ["normalise_map_direct", "normalise_sitelinks", "normalise_claims_direct",
             "atomic_sink_parquet", "check_ids", "lookup_to_long", "schema_to_dict"]:
    setattr(process, name, timed(name, getattr(process, name)))
process.pl.read_parquet = timed("pl.read_parquet", pl.read_parquet)
work = Path.cwd()
r0, t0 = resource.getrusage(resource.RUSAGE_SELF), time.perf_counter()
process.process(work / "data", work / "results", REPO_ID, work / "state", chunk_idx=0)
t1, r1 = time.perf_counter(), resource.getrusage(resource.RUSAGE_SELF)
claims = pl.read_parquet(work / "results" / "claims" / "chunk_0.parquet")
print(json.dumps({"wall": t1 - t0,
    "cpu": (r1.ru_utime - r0.ru_utime) + (r1.ru_stime - r0.ru_stime),
    "maxrss_mb": r1.ru_maxrss / 1024, "rows": claims.height,
    "steps": {k: round(v, 3) for k, v in steps.items()}}))
"""

python, chunk, runs = sys.argv[1], sys.argv[2], int(sys.argv[3])
label = sys.argv[4] if len(sys.argv) > 4 else python
for run in range(runs):
    with tempfile.TemporaryDirectory() as work:
        mirror = Path(work) / "data"
        if not os.environ.get("WIKIDATA_RELEASE"):
            mirror = mirror / "huggingface_hub" / "philippesaade" / "wikidata" / "data"
        mirror.mkdir(parents=True)
        shutil.copy(chunk, mirror / "chunk_0.parquet")
        p = subprocess.run(
            [python, "-I", "-c", ONE_RUN],
            capture_output=True,
            text=True,
            cwd=work,
            env=os.environ,
        )
        if p.returncode == 0 and os.environ.get("KEEP"):
            kept = Path(os.environ["KEEP"]) / label.replace(" ", "_") / str(run)
            shutil.copytree(Path(work) / "results", kept / "results")
            for quarantine in Path(work).rglob("quarantine"):
                shutil.copytree(quarantine, kept / "quarantine")
    if p.returncode != 0:
        sys.exit(p.stdout[-2000:] + p.stderr[-3000:])
    out = json.loads(p.stdout.strip().splitlines()[-1])
    stages = {}
    for m in re.finditer(r"\[profile\] ([\w+ ]+): ([0-9.]+)s", p.stderr):
        stages[m.group(1)] = round(stages.get(m.group(1), 0) + float(m.group(2)), 3)
    print(json.dumps({"label": label, "run": run, **out, "stages": stages}), flush=True)
