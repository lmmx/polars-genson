"""Every table process() wrote for a chunk, compared between two output directories:
row counts, Polars and Arrow schemas (file metadata aside), the rows (Arrow equality), and
the Parquet columns (paths, physical and logical types, levels).

Usage: python compare_outputs.py <out dir A> <out dir B> <out.json>
"""
import json
import sys
from pathlib import Path

import polars as pl
import pyarrow.parquet as pq

a_dir, b_dir, out = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]


def columns(f):
    s = f.schema
    return [(c.path, c.physical_type, str(c.logical_type), c.max_definition_level, c.max_repetition_level)
            for c in (s.column(i) for i in range(len(s)))]


result = {}
for a in sorted(a_dir.rglob("*.parquet")):
    rel = a.relative_to(a_dir)
    b = b_dir / rel
    fa, fb = pq.ParquetFile(a), pq.ParquetFile(b)
    ta, tb = (pq.read_table(p).replace_schema_metadata(None) for p in (a, b))
    r = {
        "rows": [ta.num_rows, tb.num_rows],
        "polars_schema_equal": pl.read_parquet_schema(a) == pl.read_parquet_schema(b),
        "arrow_schema_equal": ta.schema.equals(tb.schema),
        "parquet_columns_equal": columns(fa) == columns(fb),
        "rows_equal": ta.schema.equals(tb.schema) and ta.equals(tb),
        "mb": [round(a.stat().st_size / 1e6, 1), round(b.stat().st_size / 1e6, 1)],
        "created_by": [fa.metadata.created_by.split(" (build")[0], fb.metadata.created_by.split(" (build")[0]],
    }
    if not r["arrow_schema_equal"]:
        r["arrow_a"], r["arrow_b"] = str(ta.schema)[:1500], str(tb.schema)[:1500]
    if r["arrow_schema_equal"] and not r["rows_equal"]:
        r["polars_frames_equal"] = pl.read_parquet(a).equals(pl.read_parquet(b)) if ta.num_rows < 1e6 and str(rel).find("claims/") < 0 else None
    result[str(rel)] = r
    print(rel, {k: v for k, v in r.items() if not k.startswith("arrow_")}, flush=True)
json.dump(result, open(out, "w"), indent=1)
