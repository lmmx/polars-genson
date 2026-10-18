"""Compare conformed claims tables (repro_wpq_required.py OUT=...) value by value.

Reports, for each pair against the first file: whether the tables are equal as they are,
and whether they are equal once every struct whose fields are all null (at any depth) is
read as a null. Equal only after that means the files differ only in how an absence is
stored (null against a struct of nulls), with every value the same. Also counts the
structs of nulls in each file.

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


tables = {p: pl.read_parquet(p).sort("id") for p in sys.argv[1:]}
rows = {p: t.to_dicts() for p, t in tables.items()}
first = sys.argv[1]
for p, r in rows.items():
    print(
        json.dumps(
            {
                "file": p,
                "rows": len(r),
                "structs_of_nulls": structs_of_nulls(r),
                "equal_to_first": r == rows[first],
                "equal_to_first_reading_structs_of_nulls_as_null": canonical(r)
                == canonical(rows[first]),
            }
        ),
        flush=True,
    )
