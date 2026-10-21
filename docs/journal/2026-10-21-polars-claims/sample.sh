#!/usr/bin/env bash
# Poor man's profiler: start Polars' write of claims (scale.py, all 10,000 rows), then every
# 0.4 s attach gdb and take every thread's backtrace; samples go to samples.txt.
cd /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/plclaims
/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/env-wpq-rel/bin/python -I scale.py /tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/flat/claims.parquet 10000 > scale_full.txt &
PID=$!
sleep 3   # past the read
: > samples.txt
for i in $(seq 1 35); do
  kill -0 $PID 2>/dev/null || break
  echo "=== SAMPLE $i" >> samples.txt
  gdb -p $PID -batch -nx -ex "set pagination off" -ex "thread apply all bt 40" >> samples.txt 2>/dev/null
  sleep 0.4
done
wait $PID
