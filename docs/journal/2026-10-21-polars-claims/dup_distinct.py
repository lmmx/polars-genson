"""As dup.py, but each copy of the claims column is read from the file separately, so no two
top-level columns share buffers."""
import json, resource, sys, time
import polars as pl
src, copies = sys.argv[1], int(sys.argv[2])
df = pl.read_parquet(src)
df = pl.concat([df] + [pl.read_parquet(src).select(pl.col("claims").alias(f"claims_{i}")) for i in range(1, copies)], how="horizontal")
c0 = resource.getrusage(resource.RUSAGE_SELF); t = time.perf_counter()
df.lazy().sink_parquet("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/plclaims/o.parquet", compression="zstd", compression_level=1)
w = time.perf_counter() - t; c1 = resource.getrusage(resource.RUSAGE_SELF)
u, s = c1.ru_utime - c0.ru_utime, c1.ru_stime - c0.ru_stime
print(json.dumps({"copies": copies, "distinct_buffers": True, "write_s": round(w, 3), "user_s": round(u, 2), "sys_s": round(s, 2), "cores": round((u + s) / w, 2)}))
