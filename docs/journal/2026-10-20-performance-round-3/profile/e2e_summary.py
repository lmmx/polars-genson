"""Summarise e2e_runs.jsonl: per chunk and step, each sub-step's median wall and CPU over the
measured processes (warm-up process dropped), for the first in-process run ("cold": the
first call in a fresh process) and the second ("warm"), with peak RSS (VmHWM during the
sub-step) and the resting RSS before it; and polars-genson's [profile] stages (claims)."""
import json
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

R = Path(__file__).resolve().parent
rows = [json.loads(l) for l in open(R / "e2e_runs.jsonl")]
groups = defaultdict(list)
for r in rows:
    if r["run"] >= 0:
        groups[(r["chunk"], r["step"])].append(r)
for (chunk, step), rs in groups.items():
    print(f"## {chunk} {step} (n={len(rs)} processes)")
    reps = len(rs[0]["runs"])
    for rep in range(reps):
        tot = [r["runs"][rep]["wall"] for r in rs]
        cpu = [r["runs"][rep]["cpu"] for r in rs]
        print(f"  rep{rep} total wall {st.median(tot):.3f} [{min(tot):.3f}-{max(tot):.3f}] cpu {st.median(cpu):.3f}")
        for name in rs[0]["runs"][rep]["subs"]:
            w = [r["runs"][rep]["subs"][name]["wall"] for r in rs]
            c = [r["runs"][rep]["subs"][name]["cpu"] for r in rs]
            h = [r["runs"][rep]["subs"][name]["hwm_mb"] for r in rs]
            b = [r["runs"][rep]["subs"][name]["base_mb"] for r in rs]
            print(f"    {name:40s} wall {st.median(w):7.3f} [{min(w):.3f}-{max(w):.3f}] cpu {st.median(c):7.3f}  hwm {max(h):7.0f} MB (base {st.median(b):.0f}, +{max(hh - bb for hh, bb in zip(h, b)):.0f})")
    prof = defaultdict(list)
    for r in rs:
        seen = defaultdict(float)
        n = defaultdict(int)
        for name, v in r["profile"]:
            n[name] += 1
            if n[name] == reps:  # the last in-process rep
                seen[name] = v
        for k, v in seen.items():
            prof[k].append(v)
    if prof:
        print("    [profile] last rep: " + ", ".join(f"{k} {st.median(v):.3f}" for k, v in prof.items()))
    print(f"    info: {rs[0]['info']}  maxrss {st.median([r['maxrss_mb'] for r in rs]):.0f} MB")
