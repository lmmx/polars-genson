"""Write each table once with each writer and check the output against the input: the
same rows and values (pyarrow `Table.equals` after casting to the input's Arrow schema,
metadata dropped), and report the Parquet layout each writer chose.

Usage: python check.py <duel binary> <table> [<table> ...]
"""

import os
import subprocess
import sys
from pathlib import Path

import pyarrow.parquet as pq

HERE = Path(__file__).parent
binary, tables = sys.argv[1], sys.argv[2:]
out = HERE / "out"
out.mkdir(exist_ok=True)

for table in tables:
    src = HERE / "data" / f"{table}.parquet"
    expected = pq.read_table(src).replace_schema_metadata(None)
    for writer in os.environ.get("CHECK_WRITERS", "genson,polars").split(","):
        dst = out / f"{table}.{writer}.check.parquet"
        subprocess.run(
            [binary, writer, str(src), str(dst)], check=True, capture_output=True
        )
        got = pq.read_table(dst).replace_schema_metadata(None)
        same_schema = got.schema.equals(expected.schema)
        if same_schema:
            equal = got.equals(expected)
        else:
            try:
                equal = got.cast(expected.schema).equals(expected)
            except Exception:
                # pyarrow cannot cast some deeply nested lists: compare each column's
                # row hashes with Polars instead
                import polars as pl

                a, b = pl.read_parquet(dst), pl.read_parquet(src)
                equal = (
                    a.columns == b.columns
                    and all(a[c].hash().equals(b[c].hash()) for c in a.columns)
                    and "(row hashes)"
                )
        meta = pq.ParquetFile(dst).metadata
        col = meta.row_group(0).column(0)
        encodings = sorted(
            {
                e
                for i in range(meta.num_columns)
                for e in meta.row_group(0).column(i).encodings
            }
        )
        print(
            f"{table:13} {writer:7} equal={equal} same_arrow_schema={same_schema} "
            f"rows={meta.num_rows} rg={meta.num_row_groups} cols={meta.num_columns} "
            f"{dst.stat().st_size / 1e6:.2f} MB stats={col.is_stats_set} encodings={encodings}"
        )
        dst.unlink()
