#!/usr/bin/env bash
# Interleaved ABBA: parent (perf/forced-maps-interned, env-c3) A, child (this, env-p1s, the same .so) B; one fresh process per
# run, one warm-up run of each first. Usage: bench_abba.sh <chunk> <release> <rounds> <log>
set -euo pipefail
S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
chunk=$1 release=$2 rounds=$3 log=$4
run() { # side tag
  local py=$S/env-c3/bin/python; [ "$1" = child ] && py=$S/env-p1s/bin/python
  echo "{\"side\":\"$1\",\"tag\":\"$2\",\"r\":$($py -I $S/research/p1/bin/bench_one.py "$chunk" "$release" -)}" >> "$log"
}
run parent warmup; run child warmup
for i in $(seq 1 "$rounds"); do
  if (( i % 2 )); then run parent run; run child run; else run child run; run parent run; fi
done
