"""wikidata-pq's own `process()` (src/wikidata/process.py at 5ad5f96) on one routed release
chunk, unchanged, with its steps timed from outside: each call of the functions below is
wrapped and its wall time, CPU time (all threads) and peak RSS recorded. The chunk is
processed `runs` times in one process, each into fresh directories: run 1 is cold.

Usage: python profile_process.py <routed chunk.parquet> <workdir> <runs> <out.json>
"""
import functools
import importlib.metadata
import inspect
import json
import os
import resource
import sys
import time
from pathlib import Path

chunk, work, runs, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
os.environ["WIKIDATA_RELEASE"] = "20261012"
work.mkdir(parents=True, exist_ok=True)
os.chdir(work)  # config's paths (releases/, quarantine/) are relative
sys.path.insert(0, os.environ.get("WPQ_SRC", "/home/user/lmmx/wikidata-pq/src"))

import polars as pl  # noqa: E402
import polars_genson  # noqa: E402,F401
from wikidata import process as P  # noqa: E402

GENSON = importlib.metadata.version("polars-genson")
if not GENSON.startswith("0."):
    # 1.x decodes maps to Parquet maps by default; wikidata-pq's schemas are 0.9's kv lists
    P.normalise_from_parquet = functools.partial(P.normalise_from_parquet, map_encoding="kv")
    P.avro_to_polars_schema = functools.partial(P.avro_to_polars_schema, map_encoding="kv")

steps: dict = {}


def rss_kb(field: str) -> int:
    for line in open("/proc/self/status"):
        if line.startswith(field):
            return int(line.split()[1])
    return 0


def cpu() -> float:
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime


def timed(name_of):
    def wrap(f):
        @functools.wraps(f)
        def inner(*a, **k):
            name = name_of(*a, **k)
            open("/proc/self/clear_refs", "w").write("5")
            base, t, c = rss_kb("VmRSS"), time.perf_counter(), cpu()
            try:
                return f(*a, **k)
            finally:
                e = steps.setdefault(name, {"calls": 0, "wall": 0.0, "cpu": 0.0, "peak_over_base_mb": 0.0})
                e["calls"] += 1
                e["wall"] += time.perf_counter() - t
                e["cpu"] += cpu() - c
                e["peak_over_base_mb"] = max(e["peak_over_base_mb"], (rss_kb("VmHWM") - base) / 1024)
        return inner
    return wrap


def caller() -> str:
    for frame in inspect.stack()[2:8]:
        if frame.filename.endswith("process.py") and "wikidata" in frame.filename:
            return frame.function
    return "?"


P.normalise_from_parquet = timed(lambda *a, **k: f"genson.normalise_from_parquet[{k.get('column')}]")(P.normalise_from_parquet)
P.normalise_map_direct = timed(lambda *a, **k: f"process.normalise_map_direct[{k.get('key')}]")(P.normalise_map_direct)
P.normalise_sitelinks = timed(lambda *a, **k: "process.normalise_sitelinks (genson.normalise_json)")(P.normalise_sitelinks)
P.normalise_claims_direct = timed(lambda *a, **k: "process.normalise_claims_direct (all)")(P.normalise_claims_direct)
P.lookup_to_long = timed(lambda *a, **k: "process.lookup_to_long")(P.lookup_to_long)
P.atomic_sink_parquet = timed(lambda lf, dst: f"polars.sink_parquet[{Path(dst).parent.name}]")(P.atomic_sink_parquet)
pl.LazyFrame.collect = timed(lambda *a, **k: f"polars.collect[{caller()}]")(pl.LazyFrame.collect)
P.pl.read_parquet = timed(lambda *a, **k: f"polars.read_parquet[{caller()}]")(pl.read_parquet)

result = {"chunk": chunk.name, "polars": pl.__version__, "polars_genson": GENSON, "runs": []}
df0 = pl.read_parquet(chunk)
result["input"] = {"rows": df0.height, "file_mb": round(chunk.stat().st_size / 1e6, 1),
                   **{f"{c}_json_mb": round(df0[c].str.len_bytes().sum() / 1e6, 1)
                      for c in df0.columns if df0[c].dtype == pl.String and c != "id"}}
del df0
for run in range(runs):
    steps = {}
    data, output, state = (work / f"run{run}" / d for d in ("data", "out", "state"))
    for d in (data, output, state):
        d.mkdir(parents=True, exist_ok=True)
    (data / chunk.name).unlink(missing_ok=True)
    os.link(chunk, data / chunk.name) if chunk.stat().st_dev == data.stat().st_dev else (data / chunk.name).symlink_to(chunk)
    t, c = time.perf_counter(), cpu()
    P.process(data, output, repo_id="unused", state_dir=state)
    total = {"wall": time.perf_counter() - t, "cpu": cpu() - c}
    sizes = {p.parent.name: round(p.stat().st_size / 1e6, 1) for p in output.rglob("*.parquet")}
    result["runs"].append({"run": run, **total, "steps": steps, "output_mb": sizes})
    print(f"run {run}: {total['wall']:.2f}s wall, {total['cpu']:.2f}s cpu", flush=True)
result["maxrss_mb"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
json.dump(result, open(out, "w"), indent=1)
