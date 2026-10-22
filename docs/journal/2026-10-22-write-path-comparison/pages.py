"""Per leaf column of a Parquet file: values, nulls, data pages, dictionary or not, uncompressed
and compressed bytes; and the sizes the writer's transient buffers reach (dictionary indices
held as u64 until a page ends). Usage: pages.py <file>"""
import sys, pyarrow.parquet as pq
f = pq.ParquetFile(sys.argv[1]); md = f.metadata
print(f"row groups {md.num_row_groups}, rows {md.num_rows}, leaf columns {md.num_columns}")
tot = dict(vals=0, nonnull=0, unc=0, comp=0, idx=0)
rows = []
for rg in range(md.num_row_groups):
    g = md.row_group(rg)
    for c in range(g.num_columns):
        col = g.column(c); st = col.statistics
        nulls = st.null_count if st is not None and st.has_null_count else 0
        nonnull = col.num_values - nulls
        dic = col.has_dictionary_page
        rows.append((col.path_in_schema, col.num_values, nonnull, dic, col.total_uncompressed_size, col.total_compressed_size, col.physical_type))
        tot["vals"] += col.num_values; tot["nonnull"] += nonnull; tot["unc"] += col.total_uncompressed_size; tot["comp"] += col.total_compressed_size
        if dic: tot["idx"] += nonnull
print(f"level entries {tot['vals']:,}, non-null values {tot['nonnull']:,}, dictionary-encoded non-null {tot['idx']:,} (u64 indices: {8*tot['idx']/1e6:.0f} MB if held at once)")
print(f"uncompressed {tot['unc']/1e6:.1f} MB, compressed {tot['comp']/1e6:.1f} MB")
rows.sort(key=lambda r: -r[1])
for r in rows[:15]: print(f"  {r[0][:60]:60s} vals {r[1]:>10,} nonnull {r[2]:>9,} dict {int(r[3])} unc {r[4]/1e6:6.2f} MB comp {r[5]/1e6:6.2f} MB {r[6]}")
