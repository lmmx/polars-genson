"""Time ways of writing a table with Polars and pyarrow (five runs each): the conformed
claims table wikidata-pq writes with `atomic_sink_parquet` (a `results/claims/` file kept
by `bench_process.py`), with its shape, size in memory and number of leaf columns.

Usage: python sink_opts.py <table.parquet>
"""
import sys, time, tempfile, polars as pl
df = pl.read_parquet(sys.argv[1])
print(df.shape, len(str(df.schema)), "chars of schema;", df.estimated_size() // 1_000_000, "MB in memory")
def leaves(dt):
    if isinstance(dt, pl.Struct): return sum(leaves(f.dtype) for f in dt.fields)
    if isinstance(dt, (pl.List, pl.Array)): return leaves(dt.inner)
    return 1
print("leaf columns:", sum(leaves(t) for t in df.schema.values()))
with tempfile.TemporaryDirectory() as d:
    for name, fn in [
        ("sink_parquet (as wikidata-pq)", lambda p: df.lazy().sink_parquet(p)),
        ("write_parquet", lambda p: df.write_parquet(p)),
        ("write_parquet statistics=False", lambda p: df.write_parquet(p, statistics=False)),
        ("sink_parquet statistics=False", lambda p: df.lazy().sink_parquet(p, statistics=False)),
        ("write_parquet zstd 1", lambda p: df.write_parquet(p, compression_level=1)),
        ("write_parquet pyarrow", lambda p: df.write_parquet(p, use_pyarrow=True)),
    ]:
        ts = []
        for i in range(5):
            t = time.perf_counter(); fn(f"{d}/{i}.parquet"); ts.append(time.perf_counter() - t)
        print(f"{name}: {min(ts):.3f}-{max(ts):.3f}s")
