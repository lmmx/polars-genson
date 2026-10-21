"""Per-column compressed size and dictionary use of genson's (threshold 0.75) and Polars'
writes of a table. Usage: python columns.py <duel binary> <table> [<table> ...]"""

import subprocess
import sys
from pathlib import Path
import pyarrow.parquet as pq

HERE = Path(__file__).parent
binary, tables = sys.argv[1], sys.argv[2:]
for t in tables:
    files = {}
    for w in ("genson", "polars"):
        files[w] = HERE / "out" / f"{t}.{w}.columns.parquet"
        subprocess.run(
            [binary, w, str(HERE / "data" / f"{t}.parquet"), str(files[w])],
            check=True,
            capture_output=True,
        )
    g, p = (pq.ParquetFile(files[w]).metadata for w in ("genson", "polars"))
    print(t)
    for i in range(g.num_columns):

        def agg(m):
            size = sum(
                m.row_group(r).column(i).total_compressed_size
                for r in range(m.num_row_groups)
            )
            d = any(
                m.row_group(r).column(i).has_dictionary_page
                for r in range(m.num_row_groups)
            )
            return size, "dict" if d else "plain"

        (gs, gd), (ps, pd) = agg(g), agg(p)
        flag = " <<" if abs(gs - ps) > 0.05 * max(gs, ps) else ""
        print(
            f"  {g.row_group(0).column(i).path_in_schema:22} genson {gs / 1e6:7.2f} MB {gd:5}  polars {ps / 1e6:7.2f} MB {pd:5}{flag}"
        )
    for f in files.values():
        f.unlink()
