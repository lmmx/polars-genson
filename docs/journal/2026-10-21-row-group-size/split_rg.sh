#!/usr/bin/env bash
# #292 (1Mi-row row groups) against the branch (128Ki), 10 alternating rounds after a warm-up,
# each process's user and sys time split (rus.py), at defaults and with transparent huge
# pages off for the process (rus_nothp.py).
cd /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/grid
P=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python
for wrap in rus rus_nothp; do
for t in list_struct graded_tall graded_wide flat_94; do
  rows=$($P -I -c "import pyarrow.parquet as pq; print(pq.ParquetFile('/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet').metadata.num_rows)"); br=$(( (rows+15)/16 ))
  for i in $(seq 0 10); do
    for b in $( [ $((i%2)) = 0 ] && echo "write_bench_tip write_bench_rgnew" || echo "write_bench_rgnew write_bench_tip"); do
      r=$($P -I $wrap.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/bench-bin/$b /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet $br)
      [ $i -gt 0 ] && echo "$wrap $t $b $r"
    done
  done
done
done > split_rg.txt
rm -f o.parquet; echo FINISHED >> split_rg.txt
