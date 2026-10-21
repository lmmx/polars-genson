"""Chunk 0's claims through genson 1.1.0 as wikidata-pq normalises them (untargeted): the
schema in the file's metadata (genson_avro_schema, kv maps) against the file's own schema,
and the DeepDiff of the stored claims_schema against it.

Run from the directory holding routed/chunk_0.parquet.
"""
import functools, os, sys, tempfile
os.environ["WIKIDATA_RELEASE"] = "20261012"; sys.path.insert(0, "/home/user/lmmx/wikidata-pq/src")
import polars as pl
from deepdiff import DeepDiff
from polars_genson import normalise_from_parquet, read_parquet_metadata, avro_to_polars_schema, schema_to_dict
from wikidata import process as P
nfp = functools.partial(normalise_from_parquet, map_encoding="kv")
a2p = functools.partial(avro_to_polars_schema, map_encoding="kv")
with tempfile.TemporaryDirectory() as d:
    out = f"{d}/claims.parquet"
    nfp(input_path="routed/chunk_0.parquet", column="claims", output_path=out, output_column="claims", wrap_root="claims",
        **P.CLAIMS_INFERENCE_OPTIONS, max_builders=100, typed=True, keep_columns=["id"], extract_invariants=P.LABEL_INVARIANTS,
        lookup_output_path=f"{d}/lk.parquet", prune=P.QUARANTINE_FIELDS, prune_output_path=f"{d}/pr.parquet")
    from_file = pl.Schema(pl.read_parquet_schema(out)["claims"].to_schema())
    meta = a2p(read_parquet_metadata(out)["genson_avro_schema"])
    meta_claims = pl.Schema(meta)
    print("file schema == metadata schema:", from_file == meta_claims)
    diff = DeepDiff(schema_to_dict(P.claims_schema), schema_to_dict(from_file), ignore_order=True)
    print("acceptable:", P.is_acceptable_diff(diff), {k: (len(v) if hasattr(v, "__len__") else v) for k, v in diff.items()})
    for k in diff: print(" ", k, list(diff[k])[:6] if not isinstance(diff[k], dict) else list(diff[k].items())[:4])
