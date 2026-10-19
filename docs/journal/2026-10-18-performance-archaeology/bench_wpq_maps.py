"""End-to-end timing of wikidata-pq's map steps, `normalise_map_direct` for labels,
descriptions and aliases, as `process()` runs them on each chunk: one fresh process per
run, wall and CPU time of the three calls together and of each. With `MAP_ENCODING=kv`,
`normalise_from_parquet` and `avro_to_polars_schema` get `map_encoding="kv"` (polars-genson
>= 1.0 defaults to maps; wikidata-pq's expected schemas are key/value lists).

Usage: python bench_wpq_maps.py <python> <chunk.parquet> <runs> [label]
"""

import json
import os
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
    process.avro_to_polars_schema = functools.partial(
        polars_genson.avro_to_polars_schema, map_encoding="kv")
chunk, work = Path(sys.argv[1]), Path(sys.argv[2])
out = {}
r0, t0 = resource.getrusage(resource.RUSAGE_SELF), time.perf_counter()
for key, lists in (("labels", False), ("descriptions", False), ("aliases", True)):
    t = time.perf_counter()
    process.normalise_map_direct(chunk, work / f"{key}.parquet", key=key, lists=lists)
    out[key] = time.perf_counter() - t
t1, r1 = time.perf_counter(), resource.getrusage(resource.RUSAGE_SELF)
out.update(wall=t1 - t0, cpu=(r1.ru_utime - r0.ru_utime) + (r1.ru_stime - r0.ru_stime))
print(json.dumps(out))
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
    print(json.dumps({"label": label, "run": run, **out}), flush=True)
