"""A chunk-shaped corpus as wikidata-pq's dump writes a chunk: the distinct rows of a
Parquet file of `id`, labels, descriptions, aliases, sitelinks and claims, an `entity`
column added, written by Polars with zstd level 9 (`dump.write_chunk`'s settings).

Usage: python chunk_as_dumped.py <in.parquet> <out.parquet>
"""
import polars as pl, sys
f = pl.read_parquet(sys.argv[1]).unique(subset="claims", keep="first", maintain_order=True)
f = f.with_columns(entity=pl.lit('{"type":"item"}'))
f.write_parquet(sys.argv[2], compression="zstd", compression_level=9)
print(f.shape)
