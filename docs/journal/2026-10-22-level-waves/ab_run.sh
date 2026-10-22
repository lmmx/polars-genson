#!/usr/bin/env bash
# A/B of process() (prof.py, one fresh process per chunk): A = polars-genson 1.2.2 built from
# e5d2e6e, B = the same build of perf/level-waves; 3 rounds over the 11 chunks, A/B order
# alternating by chunk and round; round 1's outputs kept for comparison.
# Usage: ab_run.sh <tag> <envA> <envB>
tag=$1; A=$2; B=$3
cd /tmp/scratch/prof; mkdir -p ab_$tag/runs ab_$tag/out
find $A $B -name "*.so*" -exec cat {} + > /dev/null
for round in 1 2 3; do
  k=0
  while read n f; do
    k=$((k+1))
    if [ $(( (k + round) % 2 )) = 0 ]; then order="A B"; else order="B A"; fi
    for v in $order; do
      if [ $v = A ]; then e=$A; else e=$B; fi
      $e/bin/python -I prof.py $f w/ab_${tag}_$v $( [ -d /tmp/scratch/wpq/src ] && echo /tmp/scratch/wpq/src ) ab_$tag/runs/${n}_${v}_r$round.json > /dev/null 2>&1 || echo "$n $v r$round FAILED"
      if [ $round = 1 ]; then rm -rf ab_$tag/out/${n}_$v; mv w/ab_${tag}_$v/out ab_$tag/out/${n}_$v; fi
    done
  done < chunks.txt
done
echo done
