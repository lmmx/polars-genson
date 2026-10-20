"""Peak RSS of normalising the claims column against its input size: one fresh process per
(chunk, rows, route); the column's first N rows are read and held, VmHWM is reset
(/proc/self/clear_refs 5), the call runs, and the peak above the resting RSS is reported.
Routes: nfp = normalise_from_parquet as wikidata-pq calls it (rows written to a temp
Parquet first, so its input is a file of N rows); json = the normalise_json expression
(decode=False: JSON text); typed = normalise_json(decode=True, map_encoding="kv").
Claims options are wikidata-pq's (CLAIMS_INFERENCE_OPTIONS, wrap_root, max_builders=100).

Usage: python -I rss_claims.py <chunk> <nrows> <route> <workdir>
"""
import json
import resource
import sys
import time
from pathlib import Path

import polars as pl
import polars_genson
from polars_genson import normalise_from_parquet

chunk, n, route, work = sys.argv[1], int(sys.argv[2]), sys.argv[3], Path(sys.argv[4])
OPTS = dict(ndjson=True, map_threshold=0, unify_maps=True, force_field_types={"mainsnak": "record"},
            force_scalar_promotion={"mainsnak", "datavalue", "precision", "latitude", "longitude"},
            no_unify={"qualifiers"}, wrap_root="claims", max_builders=100)


def kb(field):
    for line in open("/proc/self/status"):
        if line.startswith(field):
            return int(line.split()[1])


df = pl.read_parquet(chunk, columns=["id", "claims"]).head(n)
nbytes = int(df["claims"].str.len_bytes().sum())
src = work / "in.parquet"
if route == "nfp":
    df.write_parquet(src)
    del df
open("/proc/self/clear_refs", "w").write("5")
base = kb("VmRSS")
t, r0 = time.perf_counter(), resource.getrusage(resource.RUSAGE_SELF)
if route == "nfp":
    normalise_from_parquet(src, "claims", work / "out.parquet", output_column="claims", **OPTS, typed=True,
                           map_encoding="kv", keep_columns=["id"],
                           extract_invariants={"labels": "id", "property-labels": "property", "unit-labels": "unit"},
                           lookup_output_path=work / "lookup.parquet",
                           prune={"mainsnak__string", "value__string", "value", "error"},
                           prune_output_path=work / "pruned.parquet")
elif route == "nfp_noextract":
    pass
elif route == "json":
    out = df.genson.normalise_json("claims", decode=False, map_encoding="kv", **OPTS)
elif route == "typed":
    out = df.genson.normalise_json("claims", decode=True, map_encoding="kv", **OPTS)
wall = time.perf_counter() - t
r1 = resource.getrusage(resource.RUSAGE_SELF)
hwm = kb("VmHWM")
print(json.dumps({"chunk": Path(chunk).name, "rows": n, "route": route, "input_mb": nbytes / 1e6,
                  "base_mb": base / 1024, "hwm_mb": hwm / 1024, "peak_over_base_mb": (hwm - base) / 1024,
                  "ratio_peak_over_input": (hwm - base) * 1024 / nbytes, "wall": wall,
                  "cpu": r1.ru_utime + r1.ru_stime - r0.ru_utime - r0.ru_stime}))
