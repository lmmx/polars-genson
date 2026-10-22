"""runs/*.json -> (1) per chunk: median process wall/CPU, peak RSS, imports; (2) steps
ranked by their share of the summed process wall (medians per chunk, summed over the 11
chunks), with CPU, cores busy and peak RSS; (3) genson's stage timings per call, summed;
(4) time in process() outside the wrapped steps."""
import glob, json, re, statistics as st
from collections import defaultdict
runs = defaultdict(list)
for f in glob.glob("runs/*_r*.json"):
    d = json.load(open(f)); runs[re.match(r"runs/(\w+)_r\d", f).group(1)].append(d)
med = st.median
print("| chunk | entities | process wall (s) | process CPU (s) | cores | peak RSS (MB) | imports (s) | minor faults |\n|---|---|---|---|---|---|---|---|")
tot_wall = 0
for c in sorted(runs, key=lambda c: (c[0] != "c", c[0], int(c[1:]) if c[1:].isdigit() else 0)):
    rs = runs[c]; w = [r["process_wall"] for r in rs]; tot_wall += med(w)
    print(f"| {c} | | {med(w):.2f} ({min(w):.2f}–{max(w):.2f}) | {med(r['process_cpu'] for r in rs):.1f} | {med(r['process_cpu'] for r in rs)/med(w):.1f} | {med(r['process_peak_rss_mb'] for r in rs):,.0f} | {med(r['import_wall'] for r in rs):.2f} | {med(r['minflt'] for r in rs):,.0f} |")
print(f"\nsum of median process wall over chunks: {tot_wall:.1f} s\n")
agg = defaultdict(lambda: {"wall": 0, "cpu": 0, "minflt": 0, "peak": 0, "stages": defaultdict(float)})
outside = 0
for c, rs in runs.items():
    names = set().union(*(r["steps"] for r in rs))
    top = 0
    for n in names:
        vals = [r["steps"][n] for r in rs if n in r["steps"]]
        a = agg[n]; a["wall"] += med(v["wall"] for v in vals); a["cpu"] += med(v["cpu"] for v in vals)
        a["minflt"] += med(v["minflt"] for v in vals); a["peak"] = max(a["peak"], max(v["peak_over_base_mb"] for v in vals))
        for s in set().union(*(v["stages"] for v in vals)):
            a["stages"][s] += med(v["stages"].get(s, 0) for v in vals)
        if not n.startswith("process.n_ids[check_ids]"):
            top += med(v["wall"] for v in vals)
    outside += med(r["process_wall"] for r in rs) - top
print("| step | wall (s) | share | CPU (s) | cores | minor faults | max peak over start (MB) |\n|---|---|---|---|---|---|---|")
for n, a in sorted(agg.items(), key=lambda x: -x[1]["wall"]):
    if a["wall"] < 0.05: continue
    print(f"| {n} | {a['wall']:.2f} | {100*a['wall']/tot_wall:.1f}% | {a['cpu']:.1f} | {a['cpu']/a['wall']:.1f} | {a['minflt']:,.0f} | {a['peak']:,.0f} |")
print(f"\nprocess() wall outside the wrapped steps (summed): {outside:.2f} s ({100*outside/tot_wall:.1f}%)\n")
print("genson stages (summed over chunks, medians):")
for n, a in sorted(agg.items(), key=lambda x: -x[1]["wall"]):
    if a["stages"]:
        print(f"  {n}: " + ", ".join(f"{s} {v:.2f}" for s, v in sorted(a["stages"].items(), key=lambda x: -x[1])))
