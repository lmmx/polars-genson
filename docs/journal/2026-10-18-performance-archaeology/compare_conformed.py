"""Compare conformed claims tables (repro_wpq_required.py OUT=...) value by value.

Reports, for each file against the first, how many rows differ as they are, and how many
still differ once every struct whose fields are all null (at any depth) is read as a
null (10 rows at a time, sorted by id). Rows that differ only before that differ only in
how an absence is stored (null against a struct of nulls), every value being the same.
Also counts the structs of nulls in each file.

Usage: python compare_conformed.py <claims.parquet> <claims.parquet> [...]
"""

import json
import sys

import polars as pl


def canonical(v):
    if isinstance(v, dict):
        c = {k: canonical(x) for k, x in v.items()}
        return None if all(x is None for x in c.values()) else c
    if isinstance(v, list):
        return [canonical(x) for x in v]
    return v


def structs_of_nulls(v):
    if isinstance(v, dict):
        own = int(all(canonical(x) is None for x in v.values()))
        return own + sum(structs_of_nulls(x) for x in v.values())
    if isinstance(v, list):
        return sum(structs_of_nulls(x) for x in v)
    return 0


first, rest = sys.argv[1], sys.argv[1:]
tables = {p: pl.read_parquet(p).sort("id") for p in rest}
heights = {p: t.height for p, t in tables.items()}
assert len(set(heights.values())) == 1, heights
stats = {
    p: {
        "structs_of_nulls": 0,
        "rows_differing": 0,
        "rows_differing_reading_structs_of_nulls_as_null": 0,
    }
    for p in rest
}
step = 10
for offset in range(0, heights[first], step):
    ref = tables[first].slice(offset, step).to_dicts()
    for p in rest:
        rows = ref if p == first else tables[p].slice(offset, step).to_dicts()
        st = stats[p]
        st["structs_of_nulls"] += structs_of_nulls(rows)
        for a, b in zip(rows, ref):
            if a != b:
                st["rows_differing"] += 1
                if canonical(a) != canonical(b):
                    st["rows_differing_reading_structs_of_nulls_as_null"] += 1
for p in rest:
    print(
        json.dumps({"file": p, "rows": heights[p], "against": first, **stats[p]}),
        flush=True,
    )
