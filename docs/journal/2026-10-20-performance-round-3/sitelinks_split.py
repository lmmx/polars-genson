"""wikidata-pq's `normalise_sitelinks` on one chunk, stage by stage, with the
alternatives that give the same frame: inference forced to a map (production) and not
forced, the expression to JSON text, Polars' `json_decode` of that text, and the typed
route (`decode=True`) cast to the production dtype. Median of 3 runs after one warm-up.

Usage: python sitelinks_split.py <chunk.parquet>
"""

import json
import statistics
import sys
import time

import polars as pl
import polars_genson  # noqa: F401

LINK = pl.Struct({"site": pl.String, "title": pl.String, "badges": pl.List(pl.String)})
RAW = pl.Struct({"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": LINK}))})
BASE = dict(ndjson=True, wrap_root="sitelinks", max_builders=100)
FORCED = dict(BASE, force_field_types={"sitelinks": "map"})

df = pl.read_parquet(sys.argv[1], columns=["id", "sitelinks"])
mb = df["sitelinks"].str.len_bytes().sum() / 1e6
print(f"{df.height} rows, {mb:.1f} MB of sitelinks JSON, polars-genson",
      polars_genson.__version__ if hasattr(polars_genson, "__version__") else "?")


def timed(name, f, runs=3):
    out = f()
    ts = []
    for _ in range(runs):
        t = time.perf_counter()
        out = f()
        ts.append(time.perf_counter() - t)
    print(f"{name:<44} {statistics.median(ts):7.3f} s  [{min(ts):.3f}-{max(ts):.3f}]")
    return out


forced = timed("infer, forced to a map (production)",
               lambda: df.genson.infer_json_schema("sitelinks", **FORCED))
unforced = timed("infer, not forced",
                 lambda: df.genson.infer_json_schema("sitelinks", **BASE))
print("  same schema:", json.dumps(forced, sort_keys=True) == json.dumps(unforced, sort_keys=True))
text = timed("expression to JSON text (kv), forced",
             lambda: df.genson.normalise_json("sitelinks", decode=False, map_encoding="kv", **FORCED))
timed("json_decode of that text alone",
      lambda: text.to_frame().select(pl.col(text.name).str.json_decode(RAW)))
prod = timed("normalise_json(decode=RAW), forced (production)",
             lambda: df.genson.normalise_json("sitelinks", decode=RAW, **FORCED))
typed = timed("normalise_json(decode=True, kv), forced",
              lambda: df.genson.normalise_json("sitelinks", decode=True, map_encoding="kv",
                                               unnest=False, **FORCED))
print("  typed dtype:", typed.dtypes[0])
try:
    cast = typed.select(pl.col("sitelinks").cast(RAW)).unnest("sitelinks")
    print("  typed cast to RAW equals production:", cast.equals(prod.select(cast.columns)))
except Exception as e:  # noqa: BLE001
    print("  typed cast to RAW failed:", type(e).__name__, str(e)[:200])
typed_u = timed("normalise_json(decode=True, kv), not forced",
                lambda: df.genson.normalise_json("sitelinks", decode=True, map_encoding="kv",
                                                 unnest=False, **BASE))
print("  forced and unforced typed frames equal:", typed_u.equals(typed))
