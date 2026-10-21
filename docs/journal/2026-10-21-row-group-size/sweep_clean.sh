#!/usr/bin/env bash
# Row-group size sweep with transparent huge pages off for the process (rus_nothp.py), so
# the kernel's huge-page cost on this VM doesn't swamp the change: write_bench_rg
# (GENSON_RG_ROWS) at 1Mi, 512Ki, 256Ki and 128Ki rows, 5 rounds in rotated order after a
# warm-up, each process's user and sys time split.
cd /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/grid
P=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python
SIZES=(1048576 524288 262144 131072)
for t in flat_1 flat_2 flat_4 flat_8 flat_32 flat_94 list_str list_struct graded_tall graded_wide; do
  rows=$($P -I -c "import pyarrow.parquet as pq; print(pq.ParquetFile('/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet').metadata.num_rows)"); br=$(( (rows+15)/16 ))
  for i in 0 1 2 3 4 5; do
    k=$((i % 4)); order=("${SIZES[@]:k}" "${SIZES[@]:0:k}")
    for r in "${order[@]}"; do
      out=$(GENSON_RG_ROWS=$r $P -I rus_nothp.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/bench-bin/write_bench_rg /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet $br)
      [ $i -gt 0 ] && echo "$t $r $out"
    done
  done
done > sweep_clean.txt
rm -f o.parquet; echo FINISHED >> sweep_clean.txt
