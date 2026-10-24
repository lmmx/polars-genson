# Downloads each finished experiment run's timing reports and prints, per variant and
# platform, cargo's total build time (each run) and the slowest crates' durations.
import json, os, re, subprocess, sys, statistics
from collections import defaultdict
S = os.path.dirname(os.path.abspath(__file__))  # tim/<run id>/timings-<platform>/ under it
runs = json.loads(subprocess.run(["gh","api","repos/lmmx/polars-genson/actions/runs?per_page=60&event=workflow_dispatch",
    "--jq","[.workflow_runs[] | select(.name==\"CI\" and .status==\"completed\") | {id,head_branch,conclusion}]"],capture_output=True,text=True).stdout)
plats=['linux-x86_64','linux-aarch64','linux-armv7','windows-x64','macos-aarch64','macos-x86_64']
def load(f):
    s=open(f).read(); return json.loads(re.search(r'const UNIT_DATA = (\[.*?\]);\n',s,re.S).group(1))
data=defaultdict(lambda: defaultdict(list))
crates=sys.argv[1:] or ['polars-core','polars-genson-py']
for r in runs:
    d=f"{S}/tim/{r['id']}"
    if not os.path.isdir(d):
        os.makedirs(d); subprocess.run(["gh","run","download",str(r['id']),"-R","lmmx/polars-genson","-p","timings-*","-D",d],capture_output=True)
    for p in plats:
        f=f"{d}/timings-{p}/cargo-timing.html"
        if os.path.exists(f):
            u=load(f)
            row={'wall':max(x['start']+x['duration'] for x in u),'cpu':sum(x['duration'] for x in u)}
            for c in crates: row[c]=max([x['duration'] for x in u if x['name']==c] or [0])
            data[r['head_branch'].removeprefix('exp/')][p].append(row)
for k in ['wall','cpu']+crates:
    print(f"## {k} (each run; min first)")
    print('variant'.ljust(16)+''.join(p.ljust(18) for p in plats))
    for v in sorted(data):
        cells=[]
        for p in plats:
            xs=sorted(round(x[k]) for x in data[v][p])
            cells.append(('/'.join(map(str,xs)) or '-').ljust(18))
        print(v.ljust(16)+''.join(cells))
