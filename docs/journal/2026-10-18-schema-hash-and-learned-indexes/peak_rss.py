"""Peak RSS of one bench_claims inference per process, builds alternated per round.

Reads each child's ru_maxrss with os.wait4, the counter `/usr/bin/time -v` reports as
"Maximum resident set size". Usage: python peak_rss.py <dir> <rounds>, with
<dir>/bench_{base,p2compat} and <dir>/data/{rows200,labels,narrow200k}.ndjson.
"""

import collections
import os
import statistics
import subprocess
import sys

d, rounds = sys.argv[1], int(sys.argv[2])
cases = [
    ("rows200", True),
    ("rows200", False),
    ("labels", False),
    ("narrow200k", False),
]
rss = collections.defaultdict(list)
for r in range(rounds):
    for corpus, wikidata in cases:
        for build in ("base", "p2compat"):
            env = dict(os.environ, **({"WIKIDATA": "1"} if wikidata else {}))
            p = subprocess.Popen(
                [f"{d}/bench_{build}", f"{d}/data/{corpus}.ndjson", "1"],
                env=env,
                stdout=subprocess.PIPE,
                text=True,
            )
            out = p.stdout.read()
            _, status, usage = os.wait4(p.pid, 0)
            assert status == 0, out
            schema_hash = out.split("hash=")[1].split()[0]
            key = (corpus, "W" if wikidata else "D", build)
            rss[key].append((usage.ru_maxrss / 1024, schema_hash))
            print(r, *key, f"{usage.ru_maxrss / 1024:.1f}MB", schema_hash, flush=True)
print()
for key, vals in rss.items():
    mb = [v for v, _ in vals]
    print(
        *key,
        f"n={len(mb)} median {statistics.median(mb):.1f} MB [{min(mb):.1f}-{max(mb):.1f}]",
        {h for _, h in vals},
    )
