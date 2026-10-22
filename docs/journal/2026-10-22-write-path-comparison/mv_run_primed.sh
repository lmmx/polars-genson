#!/usr/bin/env bash
# Variants of process() (prof.py, one fresh process per chunk and variant), in an order drawn
# afresh for each chunk and round (Python's random, seeded by tag, round and chunk), each run
# preceded by primer.py: on this VM a process that needs more memory than the one before it
# freed pays for memory the host has taken back, so each run starts from the same history.
# Round 1's outputs kept. Usage: mv_run_primed.sh <tag> <first-last round> <chunks file>
#   <name=env:PQX[:GENSON_PAGE_LIMIT]> ...
S=/tmp/scratch
tag=$1; rounds="$2"; chunks=$3; shift 3; vs=("$@")
cd $S/cmp; mkdir -p mv_$tag/runs mv_$tag/out w
for v in "${vs[@]}"; do e=${v#*=}; find ${e%%:*} -name "*.so*" -exec cat {} + > /dev/null; done
for round in $(seq ${rounds%-*} ${rounds#*-}); do
  k=0
  while read c f; do
    k=$((k+1))
    mapfile -t order < <(python3 -c "import random, sys; v = sys.argv[2:]; random.Random(sys.argv[1]).shuffle(v); print('\n'.join(v))" "$tag-$round-$k" "${vs[@]}")
    for v in "${order[@]}"; do
      name=${v%%=*}; rest=${v#*=}
      IFS=: read env pqx limit <<< "$rest"
      [ "${PRIMER:-1}" = 1 ] && python3 primer.py 3
      PQX="$pqx" GENSON_PAGE_LIMIT="$limit" $env/bin/python -I $S/prof/prof.py $f w/mv_${tag}_$name $S/wpq/src mv_$tag/runs/${c}_${name}_r$round.json > /dev/null 2>&1 || echo "$c $name r$round FAILED"
      echo "$round $c $name" >> mv_$tag/order.txt
      if [ $round = ${rounds%-*} ] && [ ! -d mv_$tag/out/${c}_$name ]; then mv w/mv_${tag}_$name/out mv_$tag/out/${c}_$name; fi
      rm -rf w/mv_${tag}_$name
    done
  done < $chunks
done
echo done
