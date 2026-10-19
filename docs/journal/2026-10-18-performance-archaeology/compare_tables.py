"""Compare the tables two builds' `process()` wrote for one chunk (`bench_process.py` with
`KEEP=dir`): every Parquet file under the first run directory against the same path under
the second, by schema and then value by value (pyarrow's `Table.equals`, exact: a null
and a struct of nulls differ, as do floats and row order; Polars' `assert_frame_equal` and
`DataFrame.equals` take many minutes on a list of structs of lists).

Usage: python compare_tables.py <dir/labelA/0> <dir/labelB/0>
"""

import sys
from pathlib import Path

import polars as pl

a, b = Path(sys.argv[1]), Path(sys.argv[2])
files = sorted(p.relative_to(a) for p in a.rglob("*.parquet"))
assert files, f"no tables under {a}"
assert files == sorted(p.relative_to(b) for p in b.rglob("*.parquet")), "different tables"
for rel in files:
    x, y = pl.read_parquet(a / rel), pl.read_parquet(b / rel)
    assert x.schema == y.schema, f"{rel}: schemas differ\n{x.schema}\n{y.schema}"
    assert x.to_arrow().equals(y.to_arrow()), f"{rel}: values differ"
    print(f"{rel}: {x.height} rows, {len(x.columns)} columns, equal")
