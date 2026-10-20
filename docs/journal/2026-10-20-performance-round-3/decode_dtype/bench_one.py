"""One process: wikidata-pq's normalise_sitelinks (src/wikidata/process.py) on a chunk,
timed (wall; CPU of all threads via getrusage) with its peak RSS (VmHWM reset just before
the call via /proc/self/clear_refs, and the RSS at the reset).

Usage: python -I bench_one.py <chunk.parquet> <release:0|1> <out.parquet|->
"""
import json
import resource
import sys
import time

import polars as pl
import polars_genson  # noqa: F401

chunk, release, out = sys.argv[1], sys.argv[2] == "1", sys.argv[3]

SITELINK_FIELDS = {"site": pl.String, "title": pl.String}
if release:  # an official dump's sitelinks also list their badges (featured article, ...)
    SITELINK_FIELDS["badges"] = pl.List(pl.String)
SITELINK_SCHEMA = pl.Struct(SITELINK_FIELDS)


def normalise_sitelinks(df: pl.DataFrame) -> pl.DataFrame:
    raw = pl.Struct(
        {"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": SITELINK_SCHEMA}))}
    )
    links = df.genson.normalise_json(
        "sitelinks",
        ndjson=True,
        wrap_root="sitelinks",
        force_field_types={"sitelinks": "map"},
        decode=raw,
        max_builders=100,
    )
    return pl.concat([df.select("id"), links], how="horizontal")


def rss_kb(field):
    for line in open("/proc/self/status"):
        if line.startswith(field):
            return int(line.split()[1])


def usage():
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime, r.ru_stime, r.ru_minflt


def cpu():
    u, s, _ = usage()
    return u + s


df = pl.read_parquet(chunk, columns=["id", "sitelinks"])
open("/proc/self/clear_refs", "w").write("5")
base = rss_kb("VmRSS")
t, c, u0 = time.perf_counter(), cpu(), usage()
res = normalise_sitelinks(df)
wall, cpu_s, u1 = time.perf_counter() - t, cpu() - c, usage()
hwm = rss_kb("VmHWM")
if out != "-":
    res.write_parquet(out)
print(json.dumps({"chunk": chunk.rsplit("/", 1)[-1], "release": release, "wall": round(wall, 4),
                  "cpu": round(cpu_s, 4), "user": round(u1[0] - u0[0], 4), "sys": round(u1[1] - u0[1], 4),
                  "minflt": u1[2] - u0[2], "hwm_mb": round(hwm / 1024, 1), "base_mb": round(base / 1024, 1),
                  "peak_over_base_mb": round((hwm - base) / 1024, 1), "rows": res.height,
                  "links": int(res["sitelinks"].list.len().sum())}))
