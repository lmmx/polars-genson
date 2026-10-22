"""mv_<tag>/: every variant's round-1 outputs against the first variant's, byte for byte; per
chunk and variant, medians of process() wall, user and system CPU, the claims step's wall and
system CPU, its write stage, peak RSS, and the claims file's size.
Usage: mv_compare.py <tag> <chunks file> <variant names, the first the reference...>"""
import glob, json, re, statistics as st, sys
from collections import defaultdict
from pathlib import Path
tag, chunks_file, names = sys.argv[1], sys.argv[2], sys.argv[3:]
base = Path(f"mv_{tag}"); chunks = [l.split()[0] for l in open(chunks_file)]
ref = names[0]
for c in chunks:
    a = base / "out" / f"{c}_{ref}"
    fa = {str(p.relative_to(a)): p for p in a.rglob("*") if p.is_file()}
    for v in names[1:]:
        b = base / "out" / f"{c}_{v}"
        fb = {str(p.relative_to(b)): p for p in b.rglob("*") if p.is_file()}
        diff = [k for k in fa if k not in fb or fa[k].read_bytes() != fb[k].read_bytes()]
        print(f"{c} {v}: {len(fa)} files, " + ("all byte-identical to " + ref if not diff and fa.keys() == fb.keys() else f"{len(diff)} differ: {diff[:4]}"))
runs = defaultdict(list)
for f in glob.glob(str(base / "runs" / "*.json")):
    c, v = re.match(r".*/(\w+?)_([\w]+)_r\d+\.json", Path(f).name and f).groups(); runs[(c, v)].append(json.load(open(f)))
K = "genson.normalise_from_parquet[claims]"
def size(c, v):
    ps = list((base / "out" / f"{c}_{v}").glob("claims/*.parquet")); return sum(p.stat().st_size for p in ps) / 1e6
cols = [("process wall", lambda r: r["process_wall"]), ("process user", lambda r: r["process_cpu"] - r["process_sys"]), ("process sys", lambda r: r["process_sys"]),
        ("claims wall", lambda r: r["steps"][K]["wall"]), ("claims sys", lambda r: r["steps"][K]["sys"]), ("write stage", lambda r: r["steps"][K]["stages"].get("write_parquet", 0)),
        ("peak RSS MB", lambda r: r["process_peak_rss_mb"])]
for c in chunks:
    print(f"\n{c}  (n per variant: {[len(runs[(c, v)]) for v in names]})")
    print("| variant | " + " | ".join(n for n, _ in cols) + " | claims MB | wall range |\n|---|" + "---|" * (len(cols) + 2))
    for v in names:
        rs = runs[(c, v)]
        if not rs: continue
        cells = [f"{st.median(fn(r) for r in rs):,.0f}" if "MB" in n else f"{st.median(fn(r) for r in rs):.2f}" for n, fn in cols]
        w = sorted(r["process_wall"] for r in rs)
        print(f"| {v} | " + " | ".join(cells) + f" | {size(c, v):.2f} | {w[0]:.2f}–{w[-1]:.2f} |")

# Paired by round: each round runs every variant on a chunk back to back, so a variant's
# difference from the reference variant in the same round cancels slow drift.
import random
pref = sys.argv[3] if len(sys.argv) > 3 else names[0]
paired_ref = names[1] if len(names) > 1 and names[1] == "X0" else names[0]
def by_round(c, v):
    out = {}
    for f in glob.glob(str(base / "runs" / f"{c}_{v}_r*.json")):
        out[int(re.search(r"_r(\d+)\.json", f).group(1))] = json.load(open(f))
    return out
def ci(d):
    random.seed(0); n = len(d)
    ms = sorted(st.median(random.choices(d, k=n)) for _ in range(2000))
    return ms[100], ms[1900]
for c in chunks:
    ref = by_round(c, paired_ref)
    print(f"\n{c}: paired differences from {paired_ref} (median over rounds, 90% bootstrap interval), seconds")
    print("| variant | rounds | process wall | process user | process sys | write stage |\n|---|---|---|---|---|---|")
    for v in names:
        if v == paired_ref: continue
        r = by_round(c, v); ks = sorted(set(r) & set(ref))
        cells = []
        for n_, fn in (("wall", lambda x: x["process_wall"]), ("user", lambda x: x["process_cpu"] - x["process_sys"]), ("sys", lambda x: x["process_sys"]),
                       ("write", lambda x: x["steps"][K]["stages"].get("write_parquet", 0))):
            d = [fn(r[k]) - fn(ref[k]) for k in ks]; lo, hi = ci(d)
            cells.append(f"{st.median(d):+.2f} ({lo:+.2f}, {hi:+.2f})")
        print(f"| {v} | {len(ks)} | " + " | ".join(cells) + " |")
