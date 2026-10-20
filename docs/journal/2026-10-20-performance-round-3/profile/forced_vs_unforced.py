"""Is sitelinks' schema the same forced to a map (usual path) and not forced (interned path,
map by the default threshold or map_threshold=0)? Avro and JSON Schema compared as text."""
import sys, json, time
import polars as pl, polars_genson
f = sys.argv[1]
df = pl.read_parquet(f, columns=["sitelinks"])
base = dict(ndjson=True, wrap_root="sitelinks", max_builders=100)
out = {}
for name, extra in [("forced", dict(force_field_types={"sitelinks": "map"})), ("default", {}), ("thr0", dict(map_threshold=0)), ("unify", dict(map_threshold=0, unify_maps=True))]:
    for avro in (False, True):
        t = time.perf_counter()
        s = df.genson.infer_json_schema("sitelinks", avro=avro, **base, **extra)
        out[(name, avro)] = (json.dumps(s, sort_keys=False), time.perf_counter() - t)
for avro in (False, True):
    ref = out[("forced", avro)][0]
    for name in ("default", "thr0", "unify"):
        print(f, "avro" if avro else "json", name, "equal" if out[(name, avro)][0] == ref else "DIFFERENT",
              "sorted-equal" if json.dumps(json.loads(out[(name, avro)][0]), sort_keys=True) == json.dumps(json.loads(ref), sort_keys=True) else "sorted-different",
              f"{out[(name,avro)][1]:.3f}s vs forced {out[('forced',avro)][1]:.3f}s")
print(out[("forced", True)][0][:600])
print(out[("default", True)][0][:600])
