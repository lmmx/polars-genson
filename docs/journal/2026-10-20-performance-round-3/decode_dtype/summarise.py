"""Median (and min-max) per side of the ABBA runs in the given logs, warm-ups excluded."""
import json
import statistics as st
import sys

for log in sys.argv[1:]:
    runs = [json.loads(l) for l in open(log)]
    print(log.rsplit("/", 1)[-1])
    for side in ("parent", "child"):
        rs = [r["r"] for r in runs if r["side"] == side and r["tag"] == "run"]
        cells = []
        for k in ("wall", "cpu", "user", "sys", "hwm_mb", "peak_over_base_mb"):
            v = [r[k] for r in rs if k in r]
            if not v:
                continue
            cells.append(f"{k} {st.median(v):.3f} [{min(v):.3f}-{max(v):.3f}]")
        print(f"  {side:6} n={len(rs)}  " + "  ".join(cells))
