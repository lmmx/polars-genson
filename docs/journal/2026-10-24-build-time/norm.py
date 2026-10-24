# Wall time per run normalised by the runner's speed: the summed duration of reference
# crates whose build no variant changes (arrow-cast, arrow-ord, arrow-select, arrow-arith,
# parquet, zstd-sys build script), scaled to that platform's median reference.
import json, os, re, glob, statistics, subprocess
from collections import defaultdict
S=os.path.dirname(os.path.abspath(__file__))
runs=json.loads(subprocess.run(["gh","api","repos/lmmx/polars-genson/actions/runs?per_page=60&event=workflow_dispatch","--jq","[.workflow_runs[] | select(.name==\"CI\" and .status==\"completed\") | {id,head_branch}]"],capture_output=True,text=True).stdout)
REF={'arrow-cast','arrow-ord','arrow-select','arrow-arith','parquet'}
plats=['linux-x86_64','linux-aarch64','linux-armv7','windows-x64','macos-aarch64','macos-x86_64']
rows=defaultdict(lambda: defaultdict(list))
for r in runs:
    for p in plats:
        f=f"{S}/tim/{r['id']}/timings-{p}/cargo-timing.html"
        if not os.path.exists(f): continue
        u=json.loads(re.search(r'const UNIT_DATA = (\[.*?\]);\n',open(f).read(),re.S).group(1))
        ref=sum(max([x['duration'] for x in u if x['name']==n] or [0]) for n in REF)
        wall=max(x['start']+x['duration'] for x in u)
        rows[p][r['head_branch'].removeprefix('exp/')].append((wall,ref))
print('variant'.ljust(16)+''.join(p.ljust(16) for p in plats))
vs=sorted({v for p in rows for v in rows[p]})
for v in vs:
    cells=[]
    for p in plats:
        allref=[ref for vv in rows[p] if vv!='no-lto' for _,ref in rows[p][vv]]
        m=statistics.median(allref) if allref else 1
        xs=sorted(round(w*m/ref) for w,ref in rows[p].get(v,[]))
        cells.append(('/'.join(map(str,xs)) or '-').ljust(16))
    print(v.ljust(16)+''.join(cells))
