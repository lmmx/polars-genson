"""Flat tables whose columns repeat to a graded degree: each column's values are drawn
uniformly (by Polars' hash of the row number) from K distinct values, K in
1k, 10k, 20k, 50k, 200k and all rows distinct, as Int64, short strings ("Q<n>") and
long strings (Wikidata entity URLs). graded_tall: 2,097,152 rows of 18 columns;
graded_wide: 419,430 rows of 90 (the 18 five times over, other hashes)."""

import polars as pl

KS = [1_000, 10_000, 20_000, 50_000, 200_000, None]


def column(n, k, kind, seed):
    i = pl.int_range(n, dtype=pl.UInt64)
    v = (i.hash(seed) % k) if k else ((i * 1_000_003 + seed) % n)
    name = f"{kind}_{'unique' if k is None else k}_{seed}"
    if kind == "int":
        return v.cast(pl.Int64).alias(name)
    if kind == "short":
        return (pl.lit("Q") + v.cast(pl.String)).alias(name)
    return (pl.lit("https://www.wikidata.org/wiki/Q") + v.cast(pl.String)).alias(name)


def table(n, copies):
    return pl.select(
        [
            column(n, k, kind, s)
            for s in range(copies)
            for k in KS
            for kind in ("int", "short", "long")
        ]
    )


table(2_097_152, 1).write_parquet("graded_tall.parquet", compression="zstd")
table(419_430, 5).write_parquet("graded_wide.parquet", compression="zstd")
