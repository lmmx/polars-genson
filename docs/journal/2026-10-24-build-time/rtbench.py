# Times bench/workloads.py for each venv build, rounds rotated; prints median (min-max)
import json, subprocess, sys, statistics
from pathlib import Path
S = Path(sys.argv[1]); rounds = int(sys.argv[2]); builds = sys.argv[3:]
W = ["labels_infer", "labels_normalise", "claims_infer", "claims_parquet", "flat_parquet"]
script = "/home/user/polars-genson/bench/workloads.py"
res = {b: {w: [] for w in W} for b in builds}
for b in builds:  # warm-up
    for w in W:
        subprocess.run([f"{S}/envs/{b}/bin/python", script, w, f"{S}/bdata"], capture_output=True)
for r in range(rounds):
    order = builds[r % len(builds):] + builds[:r % len(builds)]
    for w in W:
        for b in order:
            p = subprocess.run([f"{S}/envs/{b}/bin/python", script, w, f"{S}/bdata"], capture_output=True, text=True)
            if p.returncode: print(b, w, p.stderr[-300:]); continue
            res[b][w].append(json.loads(p.stdout.splitlines()[-1]))
json.dump(res, open(f"{S}/rtbench.json", "w"))
for w in W:
    print(w)
    for b in builds:
        for m in ["user", "wall"]:
            xs = [x[m] for x in res[b][w]]
            print(f"  {b:12} {m:5} {statistics.median(xs):.3f} ({min(xs):.3f}-{max(xs):.3f})")
