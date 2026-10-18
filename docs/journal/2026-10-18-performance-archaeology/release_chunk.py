"""A release-format chunk from Wikidata's entity JSON, through wikidata-pq's own reshaping.

Fetches each item's `Special:EntityData/<id>.json` (the JSON of the official dumps, one
entity per id) and writes them as one chunk with `wikidata.dump.write_chunk`, the function
`split-dump` uses: the `claims` column is in the release format (snaktype, hashes,
statement ids, references with snaks-order, a snak's datavalue as its value plus
`datavalue_type`). Entity JSON already fetched is reused from `<out>/entities/`.

Run with WIKIDATA_RELEASE set, in an environment with wikidata-pq installed.

Usage: python release_chunk.py <out dir> <id> [<id> ...]
"""

import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

import orjson
from wikidata import dump

out = Path(sys.argv[1])
ids = sys.argv[2:]
cache = out / "entities"
cache.mkdir(parents=True, exist_ok=True)
lines = []
for qid in ids:
    path = cache / f"{qid}.json"
    if not path.exists():
        url = f"https://www.wikidata.org/wiki/Special:EntityData/{qid}.json"
        request = urllib.request.Request(url, headers={"User-Agent": dump.USER_AGENT})
        for attempt in range(6):
            try:
                with urllib.request.urlopen(request, timeout=60) as response:
                    path.write_bytes(response.read())
                break
            except urllib.error.HTTPError as e:
                if e.code != 429 or attempt == 5:
                    raise
                time.sleep(int(e.headers.get("Retry-After") or 10 * 2**attempt))
        time.sleep(2)
    entity = orjson.loads(path.read_bytes())["entities"]
    # A redirect is served under the target's id
    lines.append(orjson.dumps(next(iter(entity.values()))))
entry = dump.write_chunk(0, lines, out)
print(json.dumps({k: entry[k] for k in ("file", "rows", "bytes", "first", "last")}))
