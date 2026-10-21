"""Chunk 0 of the official 20261012 dump: wikidata-pq's tables (claims, labels,
descriptions, aliases) made as today (genson's plain typed output, then Polars scans or
casts it to the stored schema, unnests and sinks it) and by genson directly with
`schema=` and `unnest=True`. Asserts the frames, the files' Arrow schemas and the
lookup tables are equal and that the inferred schema in the metadata passes wikidata-pq's
check, and times both routes (`runs` alternating rounds; round 1 is cold).

Usage: python equiv_target.py <routed chunk.parquet> <workdir> <runs> <out.json>
"""
import functools
import json
import os
import shutil
import sys
import time
from pathlib import Path

chunk, work, runs, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), int(sys.argv[3]), sys.argv[4]
os.environ["WIKIDATA_RELEASE"] = "20261012"
sys.path.insert(0, "/home/user/lmmx/wikidata-pq/src")

import polars as pl  # noqa: E402
import pyarrow.parquet as pq  # noqa: E402
from deepdiff import DeepDiff  # noqa: E402
import polars_genson as G  # noqa: E402
from wikidata import process as P  # noqa: E402

nfp = functools.partial(G.normalise_from_parquet, map_encoding="kv")
a2p = functools.partial(G.avro_to_polars_schema, map_encoding="kv")
P.normalise_from_parquet, P.avro_to_polars_schema = nfp, a2p
P.print = lambda *a, **k: None  # normalise_map_direct prints the inferred schema

MAPS = {"labels": False, "descriptions": False, "aliases": True}


def acceptable(stored: pl.Schema, inferred: pl.Schema) -> bool:
    d1, d2 = G.schema_to_dict(stored), G.schema_to_dict(inferred)
    return d1 == d2 or P.is_acceptable_diff(DeepDiff(d1, d2, ignore_order=True))


def claims_today(d: Path) -> pl.Schema:
    claims, inferred = P.normalise_claims_direct(chunk, d / "claims.parquet", d / "lookup.parquet", d / "quarantine.parquet")
    assert acceptable(P.claims_schema, inferred)
    P.atomic_sink_parquet(claims.lazy(), d / "claims.parquet")
    return inferred


def claims_direct(d: Path) -> pl.Schema:
    tmp = d / "claims.tmp"
    nfp(input_path=chunk, column="claims", output_path=tmp, output_column="claims", wrap_root="claims",
        **P.CLAIMS_INFERENCE_OPTIONS, profile=True, max_builders=100, typed=True, keep_columns=["id"],
        extract_invariants=P.LABEL_INVARIANTS, lookup_output_path=d / "lookup_raw.parquet",
        prune=P.QUARANTINE_FIELDS, prune_output_path=d / "pruned.parquet",
        schema=P.claims_schema, unnest=True)
    inferred = pl.Schema(a2p(G.read_parquet_metadata(tmp)["genson_avro_schema"]))
    assert acceptable(P.claims_schema, inferred)
    assert G.check_schema_compatibility(inferred, P.claims_schema) == []
    tmp.rename(d / "claims.parquet")
    P.atomic_sink_parquet(P.lookup_to_long(pl.read_parquet(d / "lookup_raw.parquet")).lazy(), d / "lookup.parquet")
    return inferred


def map_today(d: Path, key: str) -> None:
    out = d / f"{key}.parquet"
    P.atomic_sink_parquet(P.normalise_map_direct(chunk, out, key=key, lists=MAPS[key]).lazy(), out)


def map_direct(d: Path, key: str) -> None:
    tmp, expected = d / f"{key}.tmp", P._map_schema(key, lists=MAPS[key])
    nfp(input_path=chunk, column=key, output_path=tmp, output_column=key, ndjson=True, wrap_root=key,
        map_threshold=0, typed=True, keep_columns=["id"], schema=expected, unnest=True)
    inferred = pl.Schema(a2p(G.read_parquet_metadata(tmp)["genson_avro_schema"]))
    assert acceptable(expected, inferred)
    assert G.check_schema_compatibility(inferred, expected) == []
    tmp.rename(d / f"{key}.parquet")


def run(route: str, d: Path) -> dict:
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    t = {}
    for key in MAPS:
        s = time.perf_counter()
        (map_today if route == "today" else map_direct)(d, key)
        t[key] = time.perf_counter() - s
    s = time.perf_counter()
    inferred = (claims_today if route == "today" else claims_direct)(d)
    t["claims"] = time.perf_counter() - s
    t["inferred"] = inferred
    return t


def arrow_schema(path: Path):
    return pq.read_schema(path).remove_metadata()


result = {"chunk": chunk.name, "polars": pl.__version__, "rounds": []}
for r in range(runs):
    rnd = {}
    for route in ("today", "direct"):
        t = run(route, work / route)
        inferred = t.pop("inferred")
        rnd[route] = t
        print(r, route, {k: round(v, 2) for k, v in t.items()}, flush=True)
    result["rounds"].append(rnd)

checks = {}
for table in [*MAPS, "claims", "lookup"]:
    a, b = work / "today" / f"{table}.parquet", work / "direct" / f"{table}.parquet"
    fa, fb = pl.read_parquet_schema(a), pl.read_parquet_schema(b)
    ta, tb = (pq.read_table(p).replace_schema_metadata(None) for p in (a, b))
    checks[table] = {
        "rows": ta.num_rows,
        "polars_schema_equal": fa == fb,
        # Polars' nested equality is slow (row by row); Arrow's needs equal schemas
        "tables_equal": ta.schema.equals(tb.schema) and ta.equals(tb),
        "arrow_schema_equal": arrow_schema(a).equals(arrow_schema(b)),
        "columns": list(fa) if len(fa) < 12 else len(fa),
        "mb_today": round(a.stat().st_size / 1e6, 1),
        "mb_direct": round(b.stat().st_size / 1e6, 1),
        "direct_metadata_keys": sorted(k.decode() for k in (pq.read_schema(b).metadata or {})),
    }
    if not checks[table]["arrow_schema_equal"]:
        checks[table]["arrow_today"] = str(arrow_schema(a))[:2000]
        checks[table]["arrow_direct"] = str(arrow_schema(b))[:2000]
    print(table, checks[table], flush=True)
result["checks"] = checks
json.dump(result, open(out, "w"), indent=1)
