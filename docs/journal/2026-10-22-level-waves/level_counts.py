"""Leaf columns of a claims file written by genson: values (level entries) per leaf column
and how many are null, summed and the largest. Usage: level_counts.py <claims parquet>"""
import sys, pyarrow.parquet as pq
m = pq.ParquetFile(sys.argv[1]).metadata
rows = [(m.row_group(0).column(i).num_values, m.row_group(0).column(i).statistics.null_count, m.row_group(0).column(i).path_in_schema) for i in range(m.num_columns)]
tot, nul = sum(r[0] for r in rows), sum(r[1] for r in rows)
print(f"{m.num_rows:,} rows, {m.num_row_groups} row group(s), {m.num_columns} leaf columns; {tot:,} level entries, {nul:,} null ({100*nul/tot:.0f}%)")
for r in sorted(rows, reverse=True)[:5]: print(f"  {r[0]:>10,} values, {r[1]:>10,} null  {r[2]}")
