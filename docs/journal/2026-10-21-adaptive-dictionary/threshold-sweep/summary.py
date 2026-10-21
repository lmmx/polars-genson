"""Per table and threshold: file size, its excess over the smallest threshold's, and a
mark where the user CPU time's range lies wholly above (+) or below (-) threshold 0.75's.
Reads logs/sweep.jsonl and logs/sweep.txt (which thresholds made the same decisions)."""

import json
import re

rows = [json.loads(line) for line in open("logs/sweep.jsonl")]
groups = {}
for line in open("logs/sweep.txt"):
    m = re.match(r"(\S+)\s+genson ([\d./]+)\s", line)
    if m:
        first = float(m.group(2).split("/")[0])
        for t in m.group(2).split("/"):
            groups.setdefault(m.group(1), {})[float(t)] = f"genson@{first}"
TH = [0.0, 0.25, 0.5, 0.75, 0.9, 1.0]
by = {}
for r in rows:
    by.setdefault((r["table"], r["name"]), []).append(r)
print(f"{'table':13}" + "".join(f"{t:>15g}" for t in TH) + "   polars")
total = dict.fromkeys(TH, 0)
worst = dict.fromkeys(TH, 0.0)
for table, g in groups.items():
    size = {t: by[(table, g[t])][0]["bytes"] for t in TH}
    user = {t: [x["user"] for x in by[(table, g[t])]] for t in TH}
    best, base = min(size.values()), user[0.75]
    cells = []
    for t in TH:
        excess = (size[t] / best - 1) * 100
        worst[t] = max(worst[t], excess)
        total[t] += size[t]
        mark = (
            "+"
            if min(user[t]) > max(base)
            else "-"
            if max(user[t]) < min(base)
            else " "
        )
        cells.append(f"{size[t] / 1e6:8.2f}{mark}({excess:+4.1f})")
    print(
        f"{table:13}"
        + "".join(f"{c:>15}" for c in cells)
        + f"  {by[(table, 'polars')][0]['bytes'] / 1e6:7.2f}"
    )
print("total MB     " + "".join(f"{total[t] / 1e6:15.2f}" for t in TH))
print("worst excess " + "".join(f"{worst[t]:14.1f}%" for t in TH))
