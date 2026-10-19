"""Split of wikidata-pq's `normalise_sitelinks` on one chunk: inference alone, the
polars-genson `normalise_json` expression without and with its decode, and Polars'
`json_decode` of the normalised strings alone; three runs in one process.

Usage: python sitelinks_split.py <chunk.parquet>
"""
import sys, time
import polars as pl, polars_genson
from wikidata.process import SITELINK_SCHEMA
df = pl.read_parquet(sys.argv[1], columns=["id", "sitelinks"])
print(df.height, "rows,", df["sitelinks"].str.len_bytes().sum() // 1_000_000, "MB of sitelinks JSON")
raw = pl.Struct({"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": SITELINK_SCHEMA}))})
kw = dict(ndjson=True, wrap_root="sitelinks", force_field_types={"sitelinks": "map"}, max_builders=100)
for i in range(3):
    t = time.perf_counter()
    schema = df.genson.infer_json_schema("sitelinks", ndjson=True, wrap_root="sitelinks", force_field_types={"sitelinks": "map"}, max_builders=100)
    t1 = time.perf_counter()
    strings = df.genson.normalise_json("sitelinks", decode=False, map_encoding="kv", **kw)
    t2 = time.perf_counter()
    decoded = df.genson.normalise_json("sitelinks", decode=raw, **kw)
    t3 = time.perf_counter()
    json_only = strings.to_frame().select(pl.col(strings.name).str.json_decode(raw))
    t4 = time.perf_counter()
    print(f"infer {t1-t:.3f}s | normalise_json without decode {t2-t1:.3f}s | with decode {t3-t2:.3f}s | json_decode alone {t4-t3:.3f}s")
