"""Trace map unification's `required` through wikidata-pq's own claims path.

Runs wikidata-pq's `normalise_claims_direct` (polars-genson `normalise_from_parquet` with
`CLAIMS_INFERENCE_OPTIONS`, typed output, label extraction and pruning, then the scan
conformed to `claims_schema`) and the schema check `process()` applies to it, on one
chunk-shaped file (`id`, `claims`). Then reports, against the raw JSON:

- the Avro schema genson stored in the typed file's metadata: which fields of `mainsnak`
  and of its `datavalue` record are non-nullable;
- the Parquet/Polars schema the check compares: whether it can express nullability;
- in the conformed claims (what `process()` writes for the claims table), how a snak
  without a `datavalue`, and a datavalue without `precision`/`latitude`/`longitude`, is
  stored: as a null, or as a struct whose fields are all null.

Run with the Python of an environment that has wikidata-pq installed (its uv.lock pins
polars-genson 0.9.6). MAP_ENCODING=kv passes `map_encoding="kv"` to
`normalise_from_parquet` (polars-genson >= 1.0 defaults to maps, which `claims_schema`
does not use). Prints JSON lines; OUT=path also writes the conformed claims to Parquet.

Usage: python repro_wpq_required.py <claims.parquet with a claims column> <work dir>
"""

import functools
import hashlib
import importlib.metadata
import json
import os
import sys
from pathlib import Path

import polars as pl
import polars_genson
from deepdiff import DeepDiff
from polars_genson import read_parquet_metadata, schema_to_dict
from wikidata import process
from wikidata.partitioning.claims import claims_base

src, work = Path(sys.argv[1]), Path(sys.argv[2])
work.mkdir(parents=True, exist_ok=True)

if os.environ.get("MAP_ENCODING") == "kv":
    process.normalise_from_parquet = functools.partial(
        polars_genson.normalise_from_parquet, map_encoding="kv"
    )

# A chunk-shaped input: wikidata-pq keeps the `id` column beside the claims
raw = pl.read_parquet(src)
chunk = work / "chunk.parquet"
raw.with_row_index("i").select(
    pl.format("Q{}", pl.col("i")).alias("id"), "claims"
).write_parquet(chunk)


def report(**kw):
    print(json.dumps(kw), flush=True)


# 1. The Avro schema genson stores: the same call as normalise_claims_direct, kept
typed = work / "typed.parquet"
process.normalise_from_parquet(
    input_path=chunk,
    column="claims",
    output_path=typed,
    output_column="claims",
    wrap_root="claims",
    **process.CLAIMS_INFERENCE_OPTIONS,
    max_builders=100,
    typed=True,
    keep_columns=["id"],
    extract_invariants=process.LABEL_INVARIANTS,
    lookup_output_path=work / "lookup.parquet",
    prune=process.QUARANTINE_FIELDS,
    prune_output_path=work / "pruned.parquet",
)
avro = json.loads(read_parquet_metadata(typed)["genson_avro_schema"])


def nullable(t):
    return isinstance(t, list) and "null" in t


def records(t, path):
    """Every Avro record under `t`, with its path."""
    if isinstance(t, list):
        for b in t:
            yield from records(b, path)
    elif isinstance(t, dict):
        kind = t.get("type")
        if kind == "record":
            yield path, t
            for f in t.get("fields", []):
                yield from records(f["type"], f"{path}.{f['name']}")
        elif kind == "array":
            yield from records(t.get("items"), f"{path}[]")
        elif kind == "map":
            yield from records(t.get("values"), f"{path}{{}}")


for path, rec in records(avro, "claims"):
    if path.endswith((".mainsnak", ".mainsnak.datavalue")):
        report(
            avro_record=path,
            non_nullable=[f["name"] for f in rec["fields"] if not nullable(f["type"])],
        )
        if path.endswith(".datavalue"):
            break

# 2. What the check compares: Polars dtypes, from the typed file's Parquet schema
arrow_nullable = None
try:
    import pyarrow.parquet as papq

    def all_nullable(field):
        import pyarrow as pa

        t = field.type
        kids = []
        if pa.types.is_struct(t):
            kids = [t.field(i) for i in range(t.num_fields)]
        elif pa.types.is_list(t) or pa.types.is_large_list(t):
            kids = [t.value_field]
        elif pa.types.is_map(t):
            kids = [t.key_field, t.item_field]
        return field.nullable and all(
            all_nullable(k) for k in kids if k.name not in ("key", "entries")
        )

    arrow_nullable = all(all_nullable(f) for f in papq.read_schema(typed))
except ImportError:
    pass
report(typed_parquet_fields_all_nullable=arrow_nullable)

# 3. The real path: normalise_claims_direct, then process()'s schema check
claims, inferred = process.normalise_claims_direct(
    chunk,
    work / "claims.parquet",
    work / "claims_labels.parquet",
    work / "quarantine.parquet",
)
d1 = schema_to_dict(process.claims_schema)
d2 = schema_to_dict(inferred)
diff = DeepDiff(d1, d2, ignore_order=True) if d1 != d2 else {}
report(
    polars_genson=importlib.metadata.version("polars-genson"),
    polars=pl.__version__,
    schema_check_passes=process.is_acceptable_diff(diff),
    schema_diff_kinds=sorted(diff.keys()) if diff else [],
    inferred_schema_sha=hashlib.sha256(
        json.dumps(d2, sort_keys=True).encode()
    ).hexdigest()[:16],
)
if out := os.environ.get("OUT"):
    claims.write_parquet(out)

# 4. Raw truth, from the JSON: snaks with and without datavalue, and which datavalues
#    hold precision / latitude / longitude
PROMOTED = ("precision", "latitude", "longitude")


def raw_snaks(statement):
    yield "mainsnak", statement["mainsnak"]
    for snaks in (statement.get("qualifiers") or {}).values():
        for s in snaks:
            yield "qualifier", s
    for ref in statement.get("references") or []:
        for snaks in (ref.get("snaks", ref) or {}).values():
            if isinstance(snaks, list):
                for s in snaks:
                    yield "reference", s


truth = {}
for row in raw["claims"]:
    for statements in json.loads(row).values():
        for st in statements:
            for kind, snak in raw_snaks(st):
                t = truth.setdefault(kind, {"snaks": 0, "no_datavalue": 0})
                t["snaks"] += 1
                dv = snak.get("datavalue")
                if dv is None:
                    t["no_datavalue"] += 1
                elif isinstance(dv, dict):
                    for p in PROMOTED:
                        if p not in dv:
                            t[f"datavalue_without_{p}"] = (
                                t.get(f"datavalue_without_{p}", 0) + 1
                            )
report(raw=truth)


# 5. The conformed claims (what process() writes): nulls against structs of nulls
def all_null(v):
    if v is None:
        return True
    if isinstance(v, dict):
        return all(all_null(x) for x in v.values())
    return False


def tally(kind, snak, out):
    t = out.setdefault(kind, {"snaks": 0})
    t["snaks"] += 1
    dv = snak.get("datavalue")

    def bump(k):
        t[k] = t.get(k, 0) + 1

    if dv is None:
        bump("datavalue_null")
        return
    if all_null(dv):
        bump("datavalue_struct_of_nulls")
        return
    for p in PROMOTED:
        if p in dv:
            if dv[p] is None:
                bump(f"{p}_null")
            elif all_null(dv[p]):
                bump(f"{p}_struct_of_nulls")


seen = {}
for claims_kv in claims["claims"].to_list():
    for entry in claims_kv or []:
        for st in entry["value"] or []:
            tally("mainsnak", st["mainsnak"], seen)
            for group in st.get("qualifiers") or []:
                for s in group["value"] or []:
                    tally("qualifier", s, seen)
            for ref in st.get("references") or []:
                for group in (
                    ref.get("snaks", ref) if isinstance(ref, dict) else ref or []
                ):
                    for s in group["value"] or []:
                        tally("reference", s, seen)
report(conformed=seen)

# 6. As published: claims_base (one row per statement, the mainsnak unnested)
pub = claims_base(claims.lazy().drop_nulls()).select("datavalue").collect()
report(
    published_statements=pub.height,
    published_datavalue_is_null=pub["datavalue"].is_null().sum(),
)
