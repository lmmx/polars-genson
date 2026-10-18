"""How many leading rows decide the map paths: map paths of the W schema inferred from the
first k rows, against those inferred from all rows.

Usage: python map_stability.py <bench_claims> <file.ndjson> <scratch dir> [k ...]
"""

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import map_paths  # noqa: E402

bench, corpus, scratch, *ks = sys.argv[1:]
ks = [int(k) for k in ks] or [10, 20, 50, 100, 200]


def paths_for(k):
    out = Path(scratch) / f"schema_first{k}.json"
    env = dict(os.environ, WIKIDATA="1", ROWS=str(k), DUMP=str(out), MB="100")
    subprocess.run([bench, corpus, "1"], env=env, check=True, capture_output=True)
    found = set()
    map_paths(json.loads(out.read_text()), (), found)
    return found


full = paths_for(10**9)
print(f"all rows: {len(full)} map paths")
for k in ks:
    found = paths_for(k)
    print(
        f"first {k:>4} rows: {len(found & full)}/{len(full)} of the map paths, {len(found - full)} extra: {sorted('/'.join(p) for p in found - full)[:3]}"
    )
