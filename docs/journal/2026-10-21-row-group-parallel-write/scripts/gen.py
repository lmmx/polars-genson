"""Synthetic tables of about 200 MB of values each, written uncompressed as inputs:
flat tables of K columns (even columns Wikidata-style id strings "Q<n>", odd columns
Int64), and two narrow nested shapes (List(String); List(Struct{a: String, b: Int64}))."""
import polars as pl

TOTAL = 200_000_000

def ids(n, seed):
    h = (pl.int_range(n, dtype=pl.UInt64) * (2654435761 + seed)) % 99_999_989
    return pl.lit("Q") + h.cast(pl.String)

def ints(n, seed):
    return ((pl.int_range(n, dtype=pl.Int64) * (40503 + seed)) % 1_000_003).alias("x")

for k in (1, 2, 4, 8, 32, 94):
    n = TOTAL // (k * 8)
    cols = [(ids(n, i) if i % 2 == 0 else ints(n, i)).alias(f"c{i}") for i in range(k)]
    pl.select(cols).write_parquet(f"flat_{k}.parquet", compression="uncompressed")
    print("flat", k, n)

# nested, narrow: 2.5M rows of 10 items
n, m = 2_500_000, 10
base = pl.select(ids(n * m, 7).alias("a"), ints(n * m, 8).alias("b"), (pl.int_range(n * m) // m).alias("g"))
base.group_by("g", maintain_order=True).agg(pl.col("a")).select(pl.col("a").alias("l")).write_parquet("list_str.parquet", compression="uncompressed")
base.group_by("g", maintain_order=True).agg(pl.struct("a", "b").alias("l")).select("l").write_parquet("list_struct.parquet", compression="uncompressed")
print("nested", n, m)
