"""Cost of a fallback: decode=<dtype> whose leaf (title as Categorical) differs from the
inferred String, on rowsw sitelinks; one fresh process per side per run (maxrss over the whole process).
Usage: python -I fallback_cost.py <chunk>"""
import json
import resource
import sys
import time

import polars as pl
import polars_genson  # noqa: F401

df = pl.read_parquet(sys.argv[1], columns=["sitelinks"])
value = pl.Struct({"site": pl.String, "title": pl.Categorical()})
raw = pl.Struct({"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": value}))})
t, c = time.perf_counter(), time.process_time()
out = df.genson.normalise_json("sitelinks", ndjson=True, wrap_root="sitelinks",
                               force_field_types={"sitelinks": "map"}, decode=raw, max_builders=100)
print(json.dumps({"wall": round(time.perf_counter() - t, 4), "cpu": round(time.process_time() - c, 4),
                  "maxrss_mb": round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024, 1), "rows": out.height}))
