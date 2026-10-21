"""Peak RSS of the claims step alone, one route per process: python mem_claims.py <chunk> <workdir> today|direct"""
import functools
import json
import os
import shutil
import sys
import time
from pathlib import Path

chunk, work, route = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), sys.argv[3]
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



def kb(field):
    return next(int(l.split()[1]) for l in open("/proc/self/status") if l.startswith(field))

shutil.rmtree(work, ignore_errors=True); work.mkdir(parents=True)
base = kb("VmRSS")
open("/proc/self/clear_refs", "w").write("5")
t = time.perf_counter()
(claims_today if route == "today" else claims_direct)(work)
print(route, f"claims {time.perf_counter() - t:.2f}s, peak RSS over base {(kb('VmHWM') - base) / 1024:.0f} MB, base {base / 1024:.0f} MB")
