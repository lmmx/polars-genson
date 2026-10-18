"""How the published wikidata-claims dataset stores an absent datavalue and absent
promoted records (precision, latitude, longitude): as a null, or as a struct whose
fields are all null.

Reads a few row groups of `snaktype`, `datavalue_type` and `datavalue` over HTTP
(range requests; nothing is downloaded whole) from each file named, at a fixed revision.

Usage: python check_published.py <revision> <file> [<file> ...] (rg=0,30 env picks row groups)
"""

import json
import os
import sys

import fsspec
import polars as pl
import pyarrow.parquet as pq

rev, files = sys.argv[1], sys.argv[2:]
groups = [int(g) for g in os.environ.get("RG", "0").split(",")]
base = f"https://huggingface.co/datasets/permutans/wikidata-claims/resolve/{rev}/"
PROMOTED = ("precision", "latitude", "longitude")


def all_null(col: pl.Expr, dtype: pl.DataType) -> pl.Expr:
    """True where a struct is null or every leaf under it is null."""
    if isinstance(dtype, pl.Struct):
        leaves = [all_null(col.struct.field(f.name), f.dtype) for f in dtype.fields]
        return col.is_null() | pl.all_horizontal(leaves)
    return col.is_null()


for name in files:
    with fsspec.open(base + name, "rb", block_size=1 << 22) as f:
        pf = pq.ParquetFile(f)
        for g in groups:
            df = pl.from_arrow(
                pf.read_row_group(
                    g, columns=["snaktype", "datavalue_type", "datavalue"]
                )
            )
            dv = pl.col("datavalue")
            dtype = df.schema["datavalue"]
            out = {"file": name, "row_group": g, "rows": df.height}
            no_value = df.filter(pl.col("snaktype") != "value")
            out["snaktype_not_value"] = no_value.height
            out["of_those_datavalue_null"] = no_value.select(dv.is_null().sum()).item()
            out["of_those_datavalue_struct_of_nulls"] = no_value.select(
                (dv.is_not_null() & all_null(dv, dtype)).sum()
            ).item()
            fields = {f.name: f.dtype for f in dtype.fields}
            valued = df.filter(dv.is_not_null())
            for p in PROMOTED:
                c = dv.struct.field(p)
                out[f"{p}_null"] = valued.select(c.is_null().sum()).item()
                out[f"{p}_struct_of_nulls"] = valued.select(
                    (c.is_not_null() & all_null(c, fields[p])).sum()
                ).item()
            print(json.dumps(out), flush=True)
