"""Per stage: samples whose busy genson threads are mostly in it, and the mean busy threads in
those samples (stage_of from stages.py). Usage: par.py <samples...>"""
import re, sys, collections, importlib.util
d = __file__.rsplit('/',1)[0]
spec = importlib.util.spec_from_file_location("o", d + "/owner_lib.py"); o = importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
src = open(d + "/stages.py").read(); ns = {"o": o}; exec(src[src.index("def stage_of"):src.index("label, topn")], ns); stage_of = ns["stage_of"]
cnt = collections.Counter(); busy = collections.Counter()
for p in sys.argv[1:]:
    for s in open(p).read().split("=== SAMPLE")[1:]:
        st = collections.Counter(); idle_stage = collections.Counter()
        for th in re.split(r"\nThread \d+ ", s)[1:]:
            if "genson-core/src" not in th and "polars-genson-py/src" not in th: continue
            fr = o.frames(th)
            if not fr: continue
            head = " ".join(f for f, _, _ in fr[:6])
            sg = stage_of(fr)
            if "futex_wait" in head or "no_work_found" in head: idle_stage[sg] += 1; continue
            st[sg] += 1
        if not st: continue
        top = st.most_common(1)[0][0]; cnt[top] += 1; busy[top] += sum(st.values())
for k, v in cnt.most_common(): print(f"  {k:12s} {v:5d} samples, {busy[k]/v:.2f} busy genson threads per sample")
