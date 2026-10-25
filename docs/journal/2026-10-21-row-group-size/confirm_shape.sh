#!/usr/bin/env bash
# The shape-aware rule on the two tables it changes: Polars wheel (defaults), #292 (1Mi-row
# row groups) and the branch, 10 rounds in rotated order after a warm-up, each process's
# user and sys time split, at defaults (rus.py) and with huge pages off (rus_nothp.py).
cd /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/grid
P=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python
LANES=(polars write_bench_tip write_bench_shape)
for wrap in rus rus_nothp; do
for t in list_str list_struct; do
  rows=$($P -I -c "import pyarrow.parquet as pq; print(pq.ParquetFile('/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet').metadata.num_rows)"); br=$(( (rows+15)/16 ))
  for i in $(seq 0 10); do
    k=$((i % 3)); order=("${LANES[@]:k}" "${LANES[@]:0:k}")
    for b in "${order[@]}"; do
      if [ $b = polars ]; then r=$($P -I $wrap.py $P -I /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/polars_write.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet)
      else r=$($P -I $wrap.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/bench-bin/$b /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/$t.parquet o.parquet $br); fi
      [ $i -gt 0 ] && echo "$wrap $t $b $r"
    done
  done
done
done > confirm_shape.txt
rm -f o.parquet; echo FINISHED >> confirm_shape.txt
