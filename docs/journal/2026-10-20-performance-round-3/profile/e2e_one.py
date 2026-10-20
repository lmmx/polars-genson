"""One process: wikidata-pq's per-column polars-genson calls (src/wikidata/process.py at
5ad5f96), run as process() runs them, each sub-step timed (wall, CPU of all threads) with
its peak RSS (VmHWM reset before each sub-step via /proc/self/clear_refs).

normalise_from_parquet / avro_to_polars_schema get map_encoding="kv": wikidata-pq locks
polars-genson 0.9.6 (kv maps); 1.0.x defaults to Parquet maps, which its expected
List(Struct{key,value}) schemas would not match.

Usage: python -I e2e_one.py <chunk.parquet> <step> <workdir> <release:0|1> <reps>
step: labels | descriptions | aliases | sitelinks | claims | all
"""
import functools
import json
import resource
import sys
import time
from pathlib import Path

import polars as pl
import polars_genson
from polars_genson import avro_to_polars_schema, normalise_from_parquet, read_parquet_metadata, schema_to_dict

chunk, step, work, release, reps = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3]), sys.argv[4] == "1", int(sys.argv[5])
nfp = functools.partial(normalise_from_parquet, map_encoding="kv")
a2p = functools.partial(avro_to_polars_schema, map_encoding="kv")


def rss_kb(field):
    for line in open("/proc/self/status"):
        if line.startswith(field):
            return int(line.split()[1])


def cpu():
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime


subs = {}


class sub:
    def __init__(self, name):
        self.name = name

    def __enter__(self):
        open("/proc/self/clear_refs", "w").write("5")
        self.base = rss_kb("VmRSS")
        self.t, self.c = time.perf_counter(), cpu()

    def __exit__(self, *a):
        e = subs.setdefault(self.name, {"wall": 0.0, "cpu": 0.0, "hwm_mb": 0.0, "base_mb": 0.0})
        e["wall"] += time.perf_counter() - self.t
        e["cpu"] += cpu() - self.c
        e["hwm_mb"] = max(e["hwm_mb"], rss_kb("VmHWM") / 1024)
        e["base_mb"] = max(e["base_mb"], self.base / 1024)


SITELINK_FIELDS = {"site": pl.String, "title": pl.String}
if release:
    SITELINK_FIELDS["badges"] = pl.List(pl.String)
SITELINK_SCHEMA = pl.Struct(SITELINK_FIELDS)
KV_SCHEMA = pl.List(pl.Struct({"key": pl.String, "value": pl.String}))
DV = {"id": pl.String, "datavalue__string": pl.String,
      "precision": pl.Struct({"precision__integer": pl.Int64, "precision__number": pl.Float64}),
      "text": pl.String, "language": pl.String, "amount": pl.String, "unit": pl.String,
      "upperBound": pl.String, "lowerBound": pl.String, "time": pl.String, "timezone": pl.Int64,
      "before": pl.Int64, "after": pl.Int64, "calendarmodel": pl.String,
      "latitude": pl.Struct({"latitude__number": pl.Float64, "latitude__integer": pl.Int64}),
      "longitude": pl.Struct({"longitude__number": pl.Float64, "longitude__integer": pl.Int64}),
      "altitude": pl.Null, "globe": pl.String}
if release:
    DV.update({"entity-type": pl.String, "numeric-id": pl.Int64})
    SNAK = pl.Struct({"snaktype": pl.String, "property": pl.String, "hash": pl.String,
                      "datavalue": pl.Struct(DV), "datavalue_type": pl.String, "datatype": pl.String})
else:
    SNAK = pl.Struct({"property": pl.String, "datavalue": pl.Struct(DV), "datatype": pl.String})
groups = pl.List(pl.Struct({"key": pl.String, "value": pl.List(SNAK)}))
if release:
    ref = pl.Struct({"hash": pl.String, "snaks": groups, "snaks-order": pl.List(pl.String)})
    stmt = pl.Struct({"mainsnak": SNAK, "type": pl.String, "id": pl.String, "rank": pl.String,
                      "references": pl.List(ref), "qualifiers": groups, "qualifiers-order": pl.List(pl.String)})
else:
    stmt = pl.Struct({"mainsnak": SNAK, "rank": pl.String, "references": pl.List(groups), "qualifiers": groups})
claims_schema = pl.Schema({"claims": pl.List(pl.Struct({"key": pl.String, "value": pl.List(stmt)}))})

CLAIMS_OPTS = {"ndjson": True, "map_threshold": 0, "unify_maps": True,
               "force_field_types": {"mainsnak": "record"},
               "force_scalar_promotion": {"mainsnak", "datavalue", "precision", "latitude", "longitude"},
               "no_unify": {"qualifiers"}}
LABEL_INVARIANTS = {"labels": "id", "property-labels": "property", "unit-labels": "unit"}
QUARANTINE = {"mainsnak__string", "value__string", "value", "error"}


def sink(df, dst):
    df.lazy().sink_parquet(dst)


def map_step(key, lists, rep):
    out = work / f"{key}_{rep}.parquet"
    with sub(f"{key}.normalise_from_parquet"):
        nfp(input_path=chunk, column=key, output_path=out, output_column=key, ndjson=True,
            wrap_root=key, map_threshold=0, typed=True, keep_columns=["id"])
    with sub(f"{key}.read_back+cast"):
        meta = read_parquet_metadata(out)
        inferred = pl.Struct(a2p(meta["genson_avro_schema"]))
        vt = pl.List(pl.String) if lists else pl.String
        expected = pl.Schema(pl.Struct({key: pl.List(pl.Struct({"key": pl.String, "value": vt}))}))
        same = schema_to_dict(expected) == schema_to_dict(pl.Schema(inferred))
        result = pl.read_parquet(out).cast({key: pl.Struct(expected)}).unnest(key)
    with sub(f"{key}.sink"):
        sink(result, work / f"{key}_out_{rep}.parquet")
    return {"schema_ok": same, "rows": result.height}


def sitelinks_step(df, rep):
    raw = pl.Struct({"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": SITELINK_SCHEMA}))})
    with sub("sitelinks.normalise_json(decode=dtype)"):
        links = df.genson.normalise_json("sitelinks", ndjson=True, wrap_root="sitelinks",
                                         force_field_types={"sitelinks": "map"}, decode=raw, max_builders=100)
        links = pl.concat([df.select("id"), links], how="horizontal")
    with sub("sitelinks.sink"):
        sink(links, work / f"links_{rep}.parquet")
    return {"rows": links.height}


def sitelinks_parts(df, rep):
    """The sitelinks step taken apart (the known facts' sanity check)."""
    raw = pl.Struct({"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": SITELINK_SCHEMA}))})
    kw = dict(ndjson=True, wrap_root="sitelinks", force_field_types={"sitelinks": "map"}, max_builders=100)
    with sub("sl.infer_forced"):
        df.genson.infer_json_schema("sitelinks", avro=True, **kw)
    with sub("sl.infer_unforced_thr0_unify"):
        df.genson.infer_json_schema("sitelinks", avro=True, ndjson=True, wrap_root="sitelinks", map_threshold=0,
                                    unify_maps=True, max_builders=100)
    with sub("sl.normalise_json_text"):
        text = df.genson.normalise_json("sitelinks", decode=False, map_encoding="kv", **kw)
    with sub("sl.json_decode_text"):
        decoded = text.str.json_decode(raw)
    with sub("sl.typed_route"):
        typed = df.genson.normalise_json("sitelinks", decode=True, map_encoding="kv", **kw)
    with sub("sl.typed_route_unforced"):
        typed2 = df.genson.normalise_json("sitelinks", decode=True, map_encoding="kv", ndjson=True, wrap_root="sitelinks",
                                          map_threshold=0, unify_maps=True, max_builders=100)
    with sub("sl.typed_select_to_dtype"):
        proj = typed.select(pl.col("sitelinks").list.eval(pl.struct(
            pl.element().struct.field("key"),
            pl.element().struct.field("value").struct.select(list(SITELINK_FIELDS)).alias("value") if False else
            pl.struct([pl.element().struct.field("value").struct.field(f) for f in SITELINK_FIELDS]).alias("value"))))
    def flat(series):
        f = series.rename("s").to_frame().select(pl.col("s").list.len().alias("n"))
        e = series.rename("s").explode().struct.unnest()
        e = e.with_columns(pl.col("value").struct.unnest()).drop("value")
        if "badges" in e.columns:
            e = e.with_columns(pl.col("badges").list.join("|"))
        return f, e
    (fa, ea), (fb, eb) = flat(proj.to_series()), flat(decoded.struct.field("sitelinks"))
    same = fa.equals(fb) and ea.equals(eb)
    return {"text_bytes": int(text.str.len_bytes().sum()), "typed_equals_decoded": bool(same),
            "typed_unforced_equals": all(a.equals(b) for a, b in zip(flat(typed2.to_series()), flat(typed.to_series())))}


def claims_step(rep):
    out, lk, pr = work / f"claims_{rep}.parquet", work / f"lookup_{rep}.parquet", work / f"pruned_{rep}.parquet"
    import io, os
    with sub("claims.normalise_from_parquet"):
        nfp(input_path=chunk, column="claims", output_path=out, output_column="claims", wrap_root="claims",
            **CLAIMS_OPTS, profile=True, max_builders=100, typed=True, keep_columns=["id"],
            extract_invariants=LABEL_INVARIANTS, lookup_output_path=lk, prune=QUARANTINE, prune_output_path=pr)
    with sub("claims.scan_conform"):
        inferred = pl.Schema(pl.read_parquet_schema(out)["claims"].to_schema())
        result = (pl.scan_parquet(out, schema={"id": pl.String, "claims": pl.Struct(claims_schema)},
                                  cast_options=pl.ScanCastOptions(missing_struct_fields="insert", extra_struct_fields="ignore"))
                  .unnest("claims").collect())
    with sub("claims.lookup_to_long"):
        lookup = pl.read_parquet(lk)
        n_lookup = lookup.height
        if not lookup.is_empty():
            maps = pl.Struct({"labels": KV_SCHEMA})
            langs = lookup.genson.normalise_json("value", ndjson=True, wrap_root="labels", map_threshold=0, decode=maps)
            long = (pl.concat([lookup.select("field", pl.col("key").alias("ref")), langs], how="horizontal")
                    .explode("labels", empty_as_null=True).unnest("labels")
                    .rename({"key": "language", "value": "label"}).drop_nulls("language"))
            sink(long, work / f"lookup_long_{rep}.parquet")
    with sub("claims.schema_check+sink"):
        ok = schema_to_dict(claims_schema) == schema_to_dict(inferred)
        sink(result, work / f"claims_out_{rep}.parquet")
    return {"rows": result.height, "lookup_rows": n_lookup, "schema_equal": ok}


df = None
if step in ("sitelinks", "all", "sitelinks_parts"):
    with sub("read_parquet(chunk)"):
        df = pl.read_parquet(chunk)
info = {}
runs = []
for rep in range(reps):
    subs = {}
    t0, c0 = time.perf_counter(), cpu()
    if step in ("labels", "all"):
        info["labels"] = map_step("labels", False, rep)
    if step in ("descriptions", "all"):
        info["descriptions"] = map_step("descriptions", False, rep)
    if step in ("aliases", "all"):
        info["aliases"] = map_step("aliases", True, rep)
    if step in ("sitelinks", "all"):
        info["sitelinks"] = sitelinks_step(df, rep)
    if step in ("claims", "all"):
        info["claims"] = claims_step(rep)
    if step == "sitelinks_parts":
        info["sitelinks_parts"] = sitelinks_parts(df, rep)
    runs.append({"wall": time.perf_counter() - t0, "cpu": cpu() - c0, "subs": subs})
print(json.dumps({"chunk": chunk.name, "step": step, "info": info, "runs": runs,
                  "maxrss_mb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024}))
