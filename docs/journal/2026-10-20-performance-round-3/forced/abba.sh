#!/bin/bash
# ABBA timing of prof3 sitelinks (parent A = 5d22498, child B = 58ad87f), one fresh process per run
S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
out=$S/fm/abba.jsonl (in the scratchpad); : > $out
for chunk in prod/rowsw release/chunk_0; do
  for mode in infer json; do
    for r in 1 2 3 4 5 6; do
      if [ $((r % 2)) = 1 ]; then order="A B"; else order="B A"; fi
      for side in $order; do
        json=$($S/fm/bin/prof3_$side $S/$chunk.parquet sitelinks $mode 1 2>/dev/null | grep "^{" | tail -1)
        echo "{\"chunk\":\"$chunk\",\"side\":\"$side\",\"r\":$json}" >> $out
      done
    done
  done
done
echo done
