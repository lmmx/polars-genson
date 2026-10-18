"""Rewrite a corpus so that every key under the given map paths becomes "*".

Objects at a map path are written with duplicate keys (`{"*": v1, "*": v2}`), which
simd-json's tape keeps in document order, so genson's builder merges all of a map's values
into one child as it reads them: the build sees the map interpretation directly instead of
one field per key. An empty map stays `{}`.

Usage: python collapse.py <in.ndjson> <final schema with maps (json)> <out.ndjson> [wrap]
`wrap` gives the field the rows are wrapped in at inference (wrap_root), e.g. "claims".
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from paths import map_paths


def dump(v, path, maps, out):
    if isinstance(v, dict):
        out.append("{")
        for i, (k, sub) in enumerate(v.items()):
            if i:
                out.append(",")
            key = "*" if path in maps else k
            out.append(json.dumps(key))
            out.append(":")
            dump(sub, (*path, key), maps, out)
        out.append("}")
    elif isinstance(v, list):
        out.append("[")
        for i, x in enumerate(v):
            if i:
                out.append(",")
            dump(x, (*path, "[]"), maps, out)
        out.append("]")
    else:
        out.append(json.dumps(v))


def main(src, schema, dst, wrap=None):
    maps = set()
    map_paths(json.loads(Path(schema).read_text()), (), maps)
    prefix = (wrap,) if wrap else ()
    with open(src) as f, open(dst, "w") as g:
        for line in f:
            out = []
            dump(json.loads(line), prefix, maps, out)
            g.write("".join(out) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
