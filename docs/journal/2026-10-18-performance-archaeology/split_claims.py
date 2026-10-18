"""Split of wikidata-pq's `normalise_claims_direct` on one chunk into the time spent in
`normalise_from_parquet`, `lookup_to_long`, `atomic_sink_parquet` and the rest, in one run
(`map_encoding="kv"` passed, for polars-genson >= 1.0).

Usage: python split_claims.py <chunk.parquet>
"""
import functools, os, sys, time, tempfile
from pathlib import Path
import polars as pl, polars_genson
from wikidata import process
times = {}
def timed(name, fn):
    def run(*a, **k):
        t = time.perf_counter(); r = fn(*a, **k); times[name] = times.get(name, 0) + time.perf_counter() - t; return r
    return run
process.normalise_from_parquet = timed("normalise_from_parquet", functools.partial(polars_genson.normalise_from_parquet, map_encoding="kv"))
process.lookup_to_long = timed("lookup_to_long", process.lookup_to_long)
process.atomic_sink_parquet = timed("atomic_sink_parquet", process.atomic_sink_parquet)
orig_scan = pl.scan_parquet
with tempfile.TemporaryDirectory() as w:
    w = Path(w); t = time.perf_counter()
    process.normalise_claims_direct(Path(sys.argv[1]), w / "c.parquet", w / "l.parquet", w / "q.parquet")
    total = time.perf_counter() - t
print({k: round(v, 3) for k, v in times.items()}, "total", round(total, 3), "rest", round(total - sum(times.values()), 3))
