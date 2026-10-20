"""Random row sets through normalise_json (decode False and True, default options and
wikidata-pq's map options), each row's scalars checked to survive (soundness_cases.lost).
A failing set is shrunk (rows dropped, then values simplified) while it still fails, and
the shrunk sets are grouped by their shape, with errors (panics, decode failures) too.

Usage: python fuzz_lossless.py <sets> [seed] [palette]  (palette: all, ints: one scalar kind)
"""

import json
import random
import sys
from collections import Counter, defaultdict

import polars as pl
import polars_genson  # noqa: F401

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from soundness_cases import lost  # noqa: E402

OPTIONS = {"default": {}, "maps": {"map_threshold": 0, "unify_maps": True}}
PALETTES = {"all": [0, 1, -7, 2.5, True, False, "s", "", "42", None], "ints": [0, 1, -7, 3, None]}
SCALARS = PALETTES[sys.argv[3] if len(sys.argv) > 3 else "all"]


def value(rng, depth):
    k = rng.randrange(9 if depth < 3 else 4)
    if k < 4:
        return rng.choice(SCALARS)
    if k < 6:
        return [value(rng, depth + 1) for _ in range(rng.randrange(0, 4))]
    return {rng.choice("abcd"): value(rng, depth + 1) for _ in range(rng.randrange(0, 4))}


def check(rows, opt, decode):
    """None if sound, else a short failure tag."""
    df = pl.DataFrame({"j": [json.dumps(r) for r in rows]})
    try:
        out = df.genson.normalise_json("j", decode=decode, unnest=False, **OPTIONS[opt])
    except Exception as e:  # noqa: BLE001
        return f"ERROR {type(e).__name__}: {str(e).splitlines()[0][:80]}"
    s = out if isinstance(out, pl.Series) else out.to_series()
    vals = s.to_list() if decode else [json.loads(v) if v else None for v in s.to_list()]
    for r, v in zip(rows, vals):
        if lost(r, v):
            return "LOST"
    return None


def simplify(v):
    if isinstance(v, dict):
        for k in list(v):
            yield {x: y for x, y in v.items() if x != k}
            for sv in simplify(v[k]):
                yield {**v, k: sv}
    elif isinstance(v, list):
        for i in range(len(v)):
            yield v[:i] + v[i + 1:]
            for sv in simplify(v[i]):
                yield v[:i] + [sv] + v[i + 1:]


def shrink(rows, opt, decode, tag):
    changed = True
    while changed:
        changed = False
        for i in range(len(rows)):
            cand = rows[:i] + rows[i + 1:]
            if cand and check(cand, opt, decode) == tag:
                rows, changed = cand, True
                break
        if changed:
            continue
        for i, r in enumerate(rows):
            for sr in simplify(r):
                cand = rows[:i] + [sr] + rows[i + 1:]
                if check(cand, opt, decode) == tag:
                    rows, changed = cand, True
                    break
            if changed:
                break
    return rows


n = int(sys.argv[1])
rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 0)
found = defaultdict(list)
tags = Counter()
for _ in range(n):
    rows = [{rng.choice("abcd"): value(rng, 0) for _ in range(rng.randrange(1, 4))}
            for _ in range(rng.randrange(1, 6))]
    opt, decode = rng.choice(list(OPTIONS)), rng.random() < 0.5
    tag = check(rows, opt, decode)
    if tag:
        tags[tag] += 1
        small = shrink(rows, opt, decode, tag)
        key = (tag, opt, decode, json.dumps(small, sort_keys=True))
        found[key[:3] + (key[3],)].append(1)
print(f"{n} sets: {sum(tags.values())} failing", dict(tags.most_common()))
for (tag, opt, decode, small), hits in sorted(found.items(), key=lambda kv: -len(kv[1])):
    print(f"{len(hits):4d}  {tag:<10} {opt:<7} decode={decode!s:<5} {small}")
