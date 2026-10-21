"""From pagesweep.jsonl: per table, the 1 MiB-page variant ("no rows/1MiB") against the
current 20,000-row pages and against Polars, by user CPU and wall time (faster or slower
only if the ranges are disjoint) and file size."""

import json
import statistics

rows = [json.loads(line) for line in open("pagesweep.jsonl")]
by = {}
for r in rows:
    by.setdefault((r["table"], r["name"]), []).append(r)
tables = list(dict.fromkeys(r["table"] for r in rows))
NEW, OLD, POLARS = "genson no rows/1MiB", "genson 20k rows/1MiB", "polars"


def verdict(a, b):
    return (
        "faster" if max(a) < min(b) else "slower" if min(a) > max(b) else "within noise"
    )


def med(xs):
    return f"{statistics.median(xs):.2f}"


print(
    "| table | user CPU s: 20k pages / 1 MiB pages / Polars | wall s: 20k / 1 MiB / Polars | MB: 20k / 1 MiB / Polars | 1 MiB vs 20k (user, wall) | 1 MiB vs Polars (user, wall) |"
)
print("|---|---|---|---|---|---|")
for t in tables:
    new, old, pol = by[(t, NEW)], by[(t, OLD)], by[(t, POLARS)]
    u = {k: [x["user"] for x in v] for k, v in (("n", new), ("o", old), ("p", pol))}
    w = {k: [x["wall"] for x in v] for k, v in (("n", new), ("o", old), ("p", pol))}
    print(
        f"| {t} | {med(u['o'])} / {med(u['n'])} / {med(u['p'])} | {med(w['o'])} / {med(w['n'])} / {med(w['p'])} | "
        f"{old[0]['bytes'] / 1e6:.2f} / {new[0]['bytes'] / 1e6:.2f} / {pol[0]['bytes'] / 1e6:.2f} | "
        f"{verdict(u['n'], u['o'])}, {verdict(w['n'], w['o'])} | {verdict(u['n'], u['p'])}, {verdict(w['n'], w['p'])} |"
    )
