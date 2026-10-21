#!/usr/bin/env bash
cd $SCRATCH/flat
for t in flat_1 flat_2 flat_4 flat_8 flat_32 flat_94 list_str list_struct claims; do
  rows=$($SCRATCH/env-wpq-rel/bin/python -I -c "import pyarrow.parquet as pq; print(pq.ParquetFile('$t.parquet').metadata.num_rows)")
  br=$(( (rows + 15) / 16 ))
  b=$($SCRATCH/bench-bin/write_bench_base $t.parquet out_base.parquet $br)
  n=$($SCRATCH/bench-bin/write_bench_new $t.parquet out_new.parquet $br)
  same=$(cmp -s out_base.parquet out_new.parquet && echo identical || echo DIFFERENT)
  ok=$($SCRATCH/env-wpq-rel/bin/python -I check.py $t.parquet out_new.parquet)
  echo "$t | base $b | new $n | bytes $same | new vs input $ok" | tee -a ab_results.txt
  rm -f out_base.parquet out_new.parquet
done
