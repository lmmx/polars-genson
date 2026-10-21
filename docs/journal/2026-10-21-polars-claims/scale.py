"""Polars wheel's sink_parquet (zstd 1) on the first n rows of chunk 0's claims, for several n:
write wall and CPU, each in a fresh process (run via scale.sh)."""
import json, resource, sys, time
import polars as pl
src, n = sys.argv[1], int(sys.argv[2])
df = pl.read_parquet(src).head(n).rechunk()
c0 = resource.getrusage(resource.RUSAGE_SELF); t = time.perf_counter()
df.lazy().sink_parquet("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/plclaims/o.parquet", compression="zstd", compression_level=1)
w = time.perf_counter() - t; c1 = resource.getrusage(resource.RUSAGE_SELF)
print(json.dumps({"rows": n, "write_s": round(w, 3), "user_s": round(c1.ru_utime - c0.ru_utime, 2), "sys_s": round(c1.ru_stime - c0.ru_stime, 2)}))
