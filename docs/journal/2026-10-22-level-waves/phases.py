"""Write samples (gdbsampler.py, bt 150) by phase: a sample's busy threads under
write_batches, the phase the most of them are in (compute_leaves, or the column writers),
and the mean number busy. Usage: phases.py <samples file> ..."""
import re, sys, collections
agg = collections.defaultdict(list)
for path in sys.argv[1:]:
    for s in open(path).read().split("=== SAMPLE")[1:]:
        wt = [t for t in re.split(r"\nThread \d+ ", s)[1:] if "write_batches" in t]
        if not wt: continue
        busy = [t for t in wt if not ("futex_wait" in "\n".join(t.split("\n")[:6]) or "no_work_found" in "\n".join(t.split("\n")[:6]))]
        ph = collections.Counter("levels (compute_leaves)" if "compute_leaves" in t else "column writers" if ("ArrowColumnWriter" in t or "GenericColumnWriter" in t) else "other" for t in busy)
        agg[ph.most_common(1)[0][0] if ph else "none busy"].append(len(busy))
for k, v in agg.items(): print(f"  {k:24s} {len(v):4d} samples, {sum(v)/len(v):.2f} busy threads of 4")
