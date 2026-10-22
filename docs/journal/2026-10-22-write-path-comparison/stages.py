"""Busy genson thread samples per stage, the stage from the innermost genson frame's source file
(tape.rs, lib.rs and phase.rs are shared, so the next genson frame outward decides), and the top
owners per stage (owner_lib rule: allocator time charged to the innermost non-std frame).
Usage: stages.py <label> <topN> <samples...>"""
import re, sys, collections, importlib.util
spec = importlib.util.spec_from_file_location("o", __file__.rsplit('/',1)[0] + "/owner_lib.py"); o = importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
def stage_of(fr):
    for f, p, _ in fr:
        if f.startswith(("__rust_", "realloc", "alloc", "malloc", "free")): continue  # the #[global_allocator] shim
        if "genson-core/src/" in p or "polars-genson-py/src/" in p:
            q = p.split("/src/", 1)[1]
            if q in ("tape.rs", "lib.rs", "phase.rs", "debug.rs"): continue
            if q.startswith("parquet"):
                ln = int(_ or 0)
                return "read" if ln < 278 or 374 <= ln < 418 else ("decode" if 838 <= ln < 869 else "write")
            if q.startswith("conform"): return "conform"
            if q.startswith("extract"): return "extract"
            if q.startswith(("genson_rs", "schema", "dag")): return "infer"
            if q.startswith("normalise"): return "decode"
            return "py:" + q if "polars-genson-py" in p else q
    return "outside genson frames"
label, topn = sys.argv[1], int(sys.argv[2])
st = collections.Counter(); own = collections.defaultdict(collections.Counter); al = collections.defaultdict(collections.Counter)
for p in sys.argv[3:]:
    for s in open(p).read().split("=== SAMPLE")[1:]:
        for th in re.split(r"\nThread \d+ ", s)[1:]:
            if "genson-core/src" not in th and "polars-genson-py/src" not in th: continue
            fr = o.frames(th)
            if not fr: continue
            head = " ".join(f for f, _, _ in fr[:6])
            if "futex_wait" in head or "no_work_found" in head: continue
            sg = stage_of(fr); st[sg] += 1
            i = next((i for i, (f, p_, _) in enumerate(fr) if not o.is_low(f, p_)), 0)
            f, p_, ln = fr[i]
            key = "zstd (C)" if "zstd/lib" in p_ else f"{f[:50]} [{p_.split('index.crates.io-1949cf8c6b5b557f/')[-1]}:{ln}]"
            own[sg][key] += 1; al[sg][key] += o.is_alloc(*fr[0][:2])
n = sum(st.values())
print(f"== {label}: {n} busy genson thread samples")
for k, v in st.most_common(): print(f"  {k:24s} {v:6d} {100*v/n:5.1f}%   alloc/memcpy innermost: {100*sum(al[k].values())/v:4.0f}%")
for sg in ("read", "decode", "infer", "extract", "conform"):
    m = sum(own[sg].values())
    if not m: continue
    print(f"\n  {sg}: top owners (share of stage, alloc share)")
    for k, v in own[sg].most_common(topn): print(f"    {100*v/m:5.1f}%  alloc {100*al[sg][k]/v:3.0f}%  {k}")
