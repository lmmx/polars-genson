"""Run a bench_claims build patched by acct_patch.py and tabulate the ACCT phase lines.

Each process runs inference `runs` times on one corpus; the ACCT lines of run k are the
k-th group (each group ends with the `total` phase). Prints the median of runs 2..n per
phase (warm), the first run's values (cold), and the process's page faults and
user/system CPU (from os.wait4).

Usage: python run_acct.py <bench binary> <corpus.ndjson> <W|D> [runs=4] [procs=1]
"""

import collections
import os
import statistics
import subprocess
import sys

bench, corpus, opt = sys.argv[1:4]
runs = int(sys.argv[4]) if len(sys.argv) > 4 else 4
procs = int(sys.argv[5]) if len(sys.argv) > 5 else 1
env = dict(os.environ, ACCT="1", **({"WIKIDATA": "1"} if opt == "W" else {}))

warm = collections.defaultdict(lambda: collections.defaultdict(list))
cold = collections.defaultdict(lambda: collections.defaultdict(list))
order = []
faults = []
for _ in range(procs):
    p = subprocess.Popen(
        [bench, corpus, str(runs)],
        env=env,
        stderr=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    err = p.stderr.read()
    out = p.stdout.read()
    _, status, ru = os.wait4(p.pid, 0)
    assert status == 0, err[-2000:]
    faults.append(
        (ru.ru_minflt, ru.ru_majflt, ru.ru_utime, ru.ru_stime, ru.ru_maxrss / 1024)
    )
    run = 0
    run_acc = {}
    for line in err.splitlines():
        if not line.startswith("ACCT "):
            continue
        parts = line.split()
        name = parts[1]
        # chunk phases repeat within a run: summed over the run's chunks
        key = name
        fields = {k: float(v) for k, v in (kv.split("=") for kv in parts[2:])}
        if key not in order:
            order.append(key)
        acc = run_acc.setdefault(key, collections.Counter())
        acc.update(fields)
        if name == "total":
            target = cold if run == 0 else warm
            for k2, a in run_acc.items():
                for f, v in a.items():
                    target[k2][f].append(v)
            run_acc.clear()
            run += 1
    hashes = {ln.split("hash=")[1] for ln in out.splitlines() if "hash=" in ln}

print(
    f"{os.path.basename(corpus)} {opt}: {procs} process(es) x {runs} runs; output hashes {hashes}"
)
cols = ["wall", "user", "sys", "thread_s", "faults", "allocs", "bytes"]
for key in order:
    w = warm[key]
    c = cold[key]
    cells = []
    for col in cols:
        if col in w:
            med = statistics.median(w[col])
            if col in ("allocs", "bytes", "faults"):
                if med:
                    cells.append(f"{col}={int(med):,}")
            else:
                cells.append(f"{col}={med:.3f}")
    cold_wall = c.get("wall") or c.get("thread_s")
    print(
        f"  {key:22} warm {' '.join(cells)}   cold {cold_wall[0] if cold_wall else 0:.4g}s"
    )
for minflt, majflt, ut, st, rss in faults:
    print(
        f"  process: minor faults {minflt:,}, major {majflt}, user {ut:.2f}s, sys {st:.2f}s, peak RSS {rss:.0f} MB"
    )
