"""Avro schemas and normalised output of wikidata-pq's genson calls (process.py at 5ad5f96)
on one chunk, written as JSON lines for comparison across builds."""
import json, sys, hashlib
import polars as pl, polars_genson  # noqa: F401
chunk = sys.argv[1]
df = pl.read_parquet(chunk)
CLAIMS = dict(ndjson=True, map_threshold=0, unify_maps=True, wrap_root="claims",
              force_field_types={"mainsnak": "record"},
              force_scalar_promotion={"mainsnak", "datavalue", "precision", "latitude", "longitude"},
              no_unify={"qualifiers"}, max_builders=100)
SITE = dict(ndjson=True, wrap_root="sitelinks", force_field_types={"sitelinks": "map"}, max_builders=100)
MAPS = dict(ndjson=True, map_threshold=0, unify_maps=True, max_builders=100)
out = {}
for col, kw in [("claims", CLAIMS), ("sitelinks", SITE), ("labels", MAPS), ("descriptions", MAPS), ("aliases", MAPS)]:
    if col not in df.columns:
        continue
    kw = dict(kw)
    if "wrap_root" not in kw:
        kw["wrap_root"] = col
    schema = df.genson.infer_json_schema(col, avro=True, **kw)
    text = df.genson.normalise_json(col, decode=False, map_encoding="kv", **kw).to_list()
    h = hashlib.sha256("\n".join(t or "" for t in text).encode()).hexdigest()[:16]
    out[col] = {"schema": schema, "normalised_sha": h}
    print(col, hashlib.sha256(json.dumps(schema, sort_keys=True).encode()).hexdigest()[:16], h, flush=True)
json.dump(out, open(sys.argv[2], "w"), sort_keys=True)
