"""ab_waves/ (7 rounds on c0, s30, s70, s90; 3 on the rest) and ab_small/ (7 rounds on the
six small chunks): per chunk, medians and ranges of process() wall, the claims step's system
CPU, its write stage, peak RSS, and user CPU (CPU less system CPU) of the claims step and of
process(), A against B."""
import glob, json, re, statistics as st
from collections import defaultdict
K = "genson.normalise_from_parquet[claims]"
for tag in ("waves", "small"):
    runs = defaultdict(list)
    for f in glob.glob(f"ab_{tag}/runs/*.json"):
        c, v = re.match(r".*/(\w+?)_([AB])_r\d\.json", f).groups(); runs[(c, v)].append(json.load(open(f)))
    print(f"## ab_{tag}\n\n| chunk | runs | process wall, median (range) A → B | claims sys A → B | write stage A → B | peak RSS MB A → B | claims user CPU A → B | process user CPU A → B |\n|---|---|---|---|---|---|---|---|")
    for c in sorted({c for c, _ in runs}, key=lambda c: (c[0] != "c", c[0], int(c[1:]))):
        def s(v, fn): xs = sorted(fn(r) for r in runs[(c, v)]); return st.median(xs), xs[0], xs[-1]
        w = [s(v, lambda r: r["process_wall"]) for v in "AB"]
        y = [s(v, lambda r: r["steps"][K]["sys"])[0] for v in "AB"]
        x = [s(v, lambda r: r["steps"][K]["stages"]["write_parquet"])[0] for v in "AB"]
        p = [s(v, lambda r: r["process_peak_rss_mb"])[0] for v in "AB"]
        u = [s(v, lambda r: r["steps"][K]["cpu"] - r["steps"][K]["sys"])[0] for v in "AB"]
        pu = [s(v, lambda r: r["process_cpu"] - r["process_sys"])[0] for v in "AB"]
        print(f"| {c} | {len(runs[(c,'A')])}+{len(runs[(c,'B')])} | {w[0][0]:.2f} ({w[0][1]:.2f}–{w[0][2]:.2f}) → {w[1][0]:.2f} ({w[1][1]:.2f}–{w[1][2]:.2f}) | {y[0]:.2f} → {y[1]:.2f} | {x[0]:.2f} → {x[1]:.2f} | {p[0]:,.0f} → {p[1]:,.0f} | {u[0]:.2f} → {u[1]:.2f} | {pu[0]:.2f} → {pu[1]:.2f} |")
    print()
