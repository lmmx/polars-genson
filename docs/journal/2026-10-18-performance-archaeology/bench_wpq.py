"""End-to-end timing of wikidata-pq's claims step, `normalise_claims_direct`.

Runs the function once per fresh process (as `process()` runs it once per chunk), in the
environment of the Python given, and reports the wall time and the CPU time of the whole
call (user + system, all threads), with the `[profile]` stage lines polars-genson prints
(wikidata-pq passes `profile=True`). With `MAP_ENCODING=kv`, `map_encoding="kv"` is passed
(polars-genson >= 1.0 defaults to maps). `WIKIDATA_RELEASE` selects the release schema.

Usage: python bench_wpq.py <python> <chunk.parquet (id, claims)> <runs> [label]
"""

import json
import os
import re
import subprocess
import sys
import tempfile

ONE_RUN = r"""
import functools, json, os, resource, sys, time
from pathlib import Path
import polars_genson
from wikidata import process
if os.environ.get("MAP_ENCODING") == "kv":
    process.normalise_from_parquet = functools.partial(
        polars_genson.normalise_from_parquet, map_encoding="kv")
chunk, work = Path(sys.argv[1]), Path(sys.argv[2])
r0, t0 = resource.getrusage(resource.RUSAGE_SELF), time.perf_counter()
claims, inferred = process.normalise_claims_direct(
    chunk, work / "claims.parquet", work / "lookup.parquet", work / "quarantine.parquet")
t1, r1 = time.perf_counter(), resource.getrusage(resource.RUSAGE_SELF)
print(json.dumps({"wall": t1 - t0,
    "cpu": (r1.ru_utime - r0.ru_utime) + (r1.ru_stime - r0.ru_stime),
    "maxrss_mb": r1.ru_maxrss / 1024, "rows": claims.height}))
"""

python, chunk, runs = sys.argv[1], sys.argv[2], int(sys.argv[3])
label = sys.argv[4] if len(sys.argv) > 4 else python
for run in range(runs):
    with tempfile.TemporaryDirectory() as work:
        p = subprocess.run(
            [python, "-I", "-c", ONE_RUN, chunk, work],
            capture_output=True,
            text=True,
            env=os.environ,
        )
    if p.returncode != 0:
        sys.exit(p.stderr[-3000:])
    out = json.loads(p.stdout.strip().splitlines()[-1])
    stages = {
        m.group(1): float(m.group(2))
        for m in re.finditer(r"\[profile\] ([\w+ ]+): ([0-9.]+)s", p.stderr)
    }
    print(json.dumps({"label": label, "run": run, **out, "stages": stages}), flush=True)
