"""Row-group size sweep: genson's write_bench (scratch build reading GENSON_RG_ROWS, with
waves of threads x 1Mi/rows row groups, rg_rows_from_env.diff) at several row-group sizes,
against the official Polars wheel at its defaults. Per table: one warm-up per lane (output
checked against the input, then read ROUNDS_READ times by Polars and pyarrow, in rotated
order), then ROUNDS rounds of one fresh writing process per lane, order rotated.
Usage: python sweep_rg.py <table> [<table> ...]   (runs go to sweep_rg.jsonl)
"""
import json, os, subprocess, sys, time
from pathlib import Path
import polars as pl
import pyarrow.parquet as pq

S = Path("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad")
FLAT, PY, OUT = S / "flat", str(S / "env-wpq-rel/bin/python"), S / "grid" / "rg_out"
ROUNDS, ROUNDS_READ = int(os.environ.get("ROUNDS", 5)), 3
SIZES = [1 << 20, 1 << 19, 1 << 18, 1 << 17, 1 << 16, 1 << 15]
LANES = ["polars"] + [f"genson_{r}" for r in SIZES]
OUT.mkdir(exist_ok=True)


def write(lane, table, keep=False):
    src, dst = FLAT / f"{table}.parquet", OUT / f"{table}.{lane}.parquet"
    rows = pq.ParquetFile(src).metadata.num_rows
    if lane == "polars":
        cmd, env = [PY, "-I", str(FLAT / "polars_write.py"), str(src), str(dst)], None
    else:
        cmd = [str(S / "bench-bin/write_bench_rg"), str(src), str(dst), str((rows + 15) // 16)]
        env = dict(os.environ, GENSON_RG_ROWS=lane.split("_")[1])
    r = json.loads(subprocess.run(cmd, capture_output=True, text=True, check=True, env=env).stdout.strip().splitlines()[-1])
    md = pq.ParquetFile(dst).metadata
    r.update(table=table, lane=lane, bytes=dst.stat().st_size, row_groups=md.num_row_groups, footer=md.serialized_size)
    if not keep:
        dst.unlink()
    return r


def timed(f):
    t = time.perf_counter(); f(); return time.perf_counter() - t


with open(S / "grid" / "sweep_rg.jsonl", "a") as log:
    for table in sys.argv[1:]:
        for lane in LANES:
            r = write(lane, table, keep=True)
            r["warmup"] = True
            r["check"] = subprocess.run([PY, "-I", str(FLAT / "check.py"), str(FLAT / f"{table}.parquet"), str(OUT / f"{table}.{lane}.parquet")],
                                        capture_output=True, text=True, check=True).stdout.strip()
            log.write(json.dumps(r) + "\n"); log.flush()
            print(table, lane, r["check"], r["row_groups"], "rg", round(r["bytes"] / 1e6, 2), "MB", flush=True)
        for i in range(ROUNDS_READ):
            k = i % len(LANES)
            for lane in LANES[k:] + LANES[:k]:
                p = OUT / f"{table}.{lane}.parquet"
                log.write(json.dumps({"table": table, "lane": lane, "read_round": i,
                                      "polars_read_s": timed(lambda: pl.read_parquet(p)),
                                      "pyarrow_read_s": timed(lambda: pq.read_table(p))}) + "\n"); log.flush()
        for lane in LANES:
            (OUT / f"{table}.{lane}.parquet").unlink()
        for i in range(ROUNDS):
            k = i % len(LANES)
            for lane in LANES[k:] + LANES[:k]:
                r = write(lane, table)
                r["round"] = i
                log.write(json.dumps(r) + "\n"); log.flush()
        print(table, "done", flush=True)
