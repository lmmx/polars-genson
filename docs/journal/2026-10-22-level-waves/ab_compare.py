"""ab_<tag>/: (1) every output file of round 1, A against B, byte for byte; (2) per chunk,
medians over the 3 rounds of process() wall, CPU and system CPU, the claims step's wall,
CPU, system CPU and minor faults, its write stage, and peak RSS; totals over the chunks.
Usage: ab_compare.py <tag>"""
import glob, json, re, statistics as st, sys, hashlib
from collections import defaultdict
from pathlib import Path
tag = sys.argv[1]; base = Path(f"ab_{tag}")
chunks = [l.split()[0] for l in open("chunks.txt")]
bad = 0; nfiles = 0
for c in chunks:
    a, b = base / "out" / f"{c}_A", base / "out" / f"{c}_B"
    fa = {str(p.relative_to(a)): p for p in a.rglob("*") if p.is_file()}
    fb = {str(p.relative_to(b)): p for p in b.rglob("*") if p.is_file()}
    if fa.keys() != fb.keys(): print(f"{c}: file sets differ"); bad += 1; continue
    for k in fa:
        nfiles += 1
        if fa[k].read_bytes() != fb[k].read_bytes(): print(f"{c}: {k} DIFFERS"); bad += 1
print(f"outputs: {nfiles} files over {len(chunks)} chunks, {'all byte-identical' if not bad else str(bad) + ' differ'}\n")
runs = defaultdict(list)
for f in glob.glob(str(base / "runs" / "*.json")):
    c, v = re.match(r".*/(\w+?)_([AB])_r\d\.json", f).groups(); runs[(c, v)].append(json.load(open(f)))
K = "genson.normalise_from_parquet[claims]"
def m(c, v, fn): return st.median(fn(r) for r in runs[(c, v)])
cols = [("process wall", lambda r: r["process_wall"]), ("process CPU", lambda r: r["process_cpu"]), ("process sys", lambda r: r["process_sys"]),
        ("claims wall", lambda r: r["steps"][K]["wall"]), ("claims sys", lambda r: r["steps"][K]["sys"]), ("write stage", lambda r: r["steps"][K]["stages"].get("write_parquet", 0)),
        ("claims faults", lambda r: r["steps"][K]["minflt"]), ("peak RSS MB", lambda r: r["process_peak_rss_mb"])]
print("| chunk | " + " | ".join(n for n, _ in cols) + " |\n|---|" + "---|" * len(cols))
tot = defaultdict(float)
for c in chunks:
    cells = []
    for n, fn in cols:
        a, b = m(c, "A", fn), m(c, "B", fn); tot[(n, "A")] += a; tot[(n, "B")] += b
        cells.append(f"{a:,.0f} → {b:,.0f}" if "MB" in n or "faults" in n else f"{a:.2f} → {b:.2f}")
    print(f"| {c} | " + " | ".join(cells) + " |")
print("| **sum** | " + " | ".join((f"{tot[(n,'A')]:,.0f} → {tot[(n,'B')]:,.0f}" if "MB" in n or "faults" in n else f"{tot[(n,'A')]:.2f} → {tot[(n,'B')]:.2f}") for n, _ in cols) + " |")
print("\nranges of process wall (A | B):")
for c in chunks:
    wa = sorted(r["process_wall"] for r in runs[(c, "A")]); wb = sorted(r["process_wall"] for r in runs[(c, "B")])
    print(f"  {c:4s} A {wa}  B {wb}")
