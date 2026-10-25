#!/usr/bin/env bash
# Both writers with transparent huge pages disabled for the process (rus_nothp.py), genson at #292's head
# with its allocator at defaults; 1 discarded warm-up then 5 rounds alternating order.
cd /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/grid
for t in flat_1 flat_2 flat_4 flat_8 flat_32 flat_94 list_str list_struct claims; do
  rows=$(/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python -I -c "import pyarrow.parquet as pq; print(pq.ParquetFile('/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet').metadata.num_rows)"); br=$(( (rows+15)/16 ))
  for i in 0 1 2 3 4 5; do
    for lane in $( [ $((i%2)) = 0 ] && echo "p g" || echo "g p"); do
      if [ $lane = p ]; then r=$(/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python -I rus_nothp.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python -I /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/polars_write.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet)
      else r=$(/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python -I rus_nothp.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/bench-bin/write_bench_tip /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet $br); fi
      [ $i -gt 0 ] && echo "$t $lane $r"
    done
  done
done > thp_grid.txt
rm -f o.parquet; echo FINISHED >> thp_grid.txt
