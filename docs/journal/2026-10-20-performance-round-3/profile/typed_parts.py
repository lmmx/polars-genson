"""The typed route (normalise_json(decode=True)) taken apart on a column: the Rust call
(_rust_normalise_json_typed: inference, ArrowPlan batches, concat, null fix-up, Arrow to
Polars), avro_to_polars_schema, and the Polars cast to that dtype; each 5 times in one
process, median of runs 2-5. Usage: python -I typed_parts.py <chunk> <preset>
preset: sitelinks (forced) | sitelinks_unify | labels"""
import json
import statistics as st
import sys
import time

import orjson
import polars as pl
import polars_genson as pg
from polars_genson import _normalise_kwargs, avro_to_polars_schema
from polars_genson._polars_genson import normalise_json_typed

chunk, preset = sys.argv[1], sys.argv[2]
col = {"sitelinks": "sitelinks", "sitelinks_unify": "sitelinks"}.get(preset, preset)
df = pl.read_parquet(chunk, columns=[col])
base = dict(ignore_outer_array=True, ndjson=True, empty_as_null=True, coerce_strings=False, map_encoding="kv",
            profile=False, map_threshold=20, map_max_required_keys=None, unify_maps=False, no_unify=None,
            force_field_types=None, force_parent_field_types=None, force_scalar_promotion=None, wrap_scalars=True,
            wrap_root=col, no_root_map=True, max_builders=100)
if preset == "sitelinks":
    base["force_field_types"] = {"sitelinks": "map"}
elif preset == "sitelinks_unify":
    base.update(map_threshold=0, unify_maps=True)
else:
    base.update(map_threshold=0)
    base["max_builders"] = None
kw = orjson.dumps(_normalise_kwargs(**base)).decode()
times = {k: [] for k in ("rust_typed", "avro_to_polars", "cast", "to_frame_unnest")}
for i in range(5):
    t = time.perf_counter()
    series, avro = normalise_json_typed(df.get_column(col), kw)
    times["rust_typed"].append(time.perf_counter() - t)
    t = time.perf_counter()
    dtype = pl.Struct(avro_to_polars_schema(avro, map_encoding="kv"))
    times["avro_to_polars"].append(time.perf_counter() - t)
    t = time.perf_counter()
    cast = series.cast(dtype)
    times["cast"].append(time.perf_counter() - t)
    t = time.perf_counter()
    out = cast.to_frame().unnest(col)
    times["to_frame_unnest"].append(time.perf_counter() - t)
print(json.dumps({"chunk": chunk.rsplit("/", 1)[-1], "preset": preset, "same_dtype": series.dtype == dtype,
                  **{k: round(st.median(v[1:]), 4) for k, v in times.items()},
                  "first": {k: round(v[0], 4) for k, v in times.items()}}))
