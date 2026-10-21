"""Read a table, then time Polars' sink_parquet (zstd 1, defaults) with its CPU time."""
import json, resource, sys, time
import polars as pl
src, dst = sys.argv[1], sys.argv[2]
df = pl.read_parquet(src)
c0 = resource.getrusage(resource.RUSAGE_SELF); t = time.perf_counter()
df.lazy().sink_parquet(dst, compression="zstd", compression_level=1)
w = time.perf_counter() - t; c1 = resource.getrusage(resource.RUSAGE_SELF)
print(json.dumps({"write_s": round(w, 3), "cpu_s": round((c1.ru_utime + c1.ru_stime) - (c0.ru_utime + c0.ru_stime), 2)}))
