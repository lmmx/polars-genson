"""Original writer grid (scripts of the #288 journal), rerun with repeats. Per table, one
discarded warm-up per lane (its output checked against the input), then ROUNDS rounds of
one fresh process per lane, order rotated each round. Lanes: Python Polars sink_parquet
(polars_write.py: zstd 1, its default row groups), and genson's write_bench example built
from the original (pre-#288) code and from #292's head, input batches of rows/16.
Usage: python grid_a.py <table> [<table> ...]   (runs go to grid_a.jsonl)
"""
import json, os, subprocess, sys
from pathlib import Path
import pyarrow.parquet as pq

S = Path("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad")
FLAT, PY = S / "flat", str(S / "env-wpq-rel/bin/python")
ROUNDS = int(os.environ.get("ROUNDS", 5))
LANES = ["polars", "genson_original", "genson_now"]
BIN = {"genson_original": S / "bench-bin/write_bench_base", "genson_now": S / "bench-bin/write_bench_tip"}


def run(lane, table, check=False):
    src, dst = FLAT / f"{table}.parquet", S / "grid" / f"out_{lane}.parquet"
    rows = pq.ParquetFile(src).metadata.num_rows
    if lane == "polars":
        cmd = [PY, "-I", str(FLAT / "polars_write.py"), str(src), str(dst)]
    else:
        cmd = [str(BIN[lane]), str(src), str(dst), str((rows + 15) // 16)]
    r = json.loads(subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip().splitlines()[-1])
    r.update(table=table, lane=lane, bytes=dst.stat().st_size, row_groups=pq.ParquetFile(dst).metadata.num_row_groups)
    if check:
        r["check"] = subprocess.run([PY, "-I", str(FLAT / "check.py"), str(src), str(dst)],
                                    capture_output=True, text=True, check=True).stdout.strip()
    dst.unlink()
    return r


with open(S / "grid" / "grid_a.jsonl", "a") as log:
    for table in sys.argv[1:]:
        for lane in LANES:
            r = run(lane, table, check=True)
            r["warmup"] = True
            log.write(json.dumps(r) + "\n"); log.flush()
            print(table, lane, "warm-up", r["check"], flush=True)
        for i in range(ROUNDS):
            k = i % len(LANES)
            for lane in LANES[k:] + LANES[:k]:
                r = run(lane, table)
                r["round"] = i
                log.write(json.dumps(r) + "\n"); log.flush()
        print(table, "done", flush=True)
