import json, statistics as st, sys
from collections import defaultdict
w, rd, meta = defaultdict(list), defaultdict(list), {}
for l in open(sys.argv[1]):
    r = json.loads(l)
    k = (r["table"], r["lane"])
    if "read_round" in r: rd[k].append(r)
    elif r.get("warmup"): meta[k] = r
    else: w[k].append(r)
f = lambda xs: f"{st.median(xs):.3f} ({min(xs):.3f}-{max(xs):.3f})"
lanes = ["polars"] + [f"genson_{1 << s}" for s in (20, 19, 18, 17, 16, 15)]
for t in dict.fromkeys(k[0] for k in meta):
    for lane in lanes:
        m, ws, rs = meta[(t, lane)], w[(t, lane)], rd[(t, lane)]
        print(f"{t:12} {lane:15} rg {m['row_groups']:4}  {m['bytes']/1e6:7.2f} MB  wall {f([x['write_s'] for x in ws])}  cpu {st.median(x['cpu_s'] for x in ws):5.2f}  "
              f"read pl {st.median(x['polars_read_s'] for x in rs):.3f} pa {st.median(x['pyarrow_read_s'] for x in rs):.3f}  {m['check'].split()[0]}")
    print()
