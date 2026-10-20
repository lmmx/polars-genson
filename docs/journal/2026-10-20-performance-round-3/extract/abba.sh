#!/bin/bash
# ABBA timing of prof3 claims (full: extract, infer, typed), parent P (58ad87f's Rust, as
# perf/decode-dtype-typed's) against child X (perf/extract-validate-once), one process per run
S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
out=$S/ex/abba.jsonl; : > $out
for chunk in prod/rowsw release/chunk_0; do
  for r in 1 2 3 4 5 6; do
    if [ $((r % 2)) = 1 ]; then order="B X"; else order="X B"; fi
    for side in $order; do
      json=$($S/fm/bin/prof3_$side $S/$chunk.parquet claims full 1 2>/dev/null | grep "^{" | tail -1)
      echo "{\"chunk\":\"$chunk\",\"side\":\"$side\",\"r\":$json}" >> $out
    done
  done
done
