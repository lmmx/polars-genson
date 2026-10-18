#!/usr/bin/env bash
# Alternate bench_claims builds per round; one process per (corpus, options, build), 3 runs each.
# Usage: run_e2e.sh <dir> <rounds>, with <dir>/bench_{base,p2compat,p2tag} (release builds of
# genson-core/examples/bench_claims.rs) and <dir>/data/{rows200,labels,narrow200k}.ndjson.
S="$1"; ROUNDS="$2"
for r in $(seq "$ROUNDS"); do
  for case in "rows200 W" "rows200 D" "labels D" "narrow200k D"; do
    set -- $case
    for b in base p2compat p2tag; do
      if [ "$2" = W ]; then env WIKIDATA=1 "$S/bench_$b" "$S/data/$1.ndjson" 3; else "$S/bench_$b" "$S/data/$1.ndjson" 3; fi \
        | grep cpu= | sed "s/^/$r $1 $2 $b /"
    done
  done
done
