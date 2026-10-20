#!/bin/bash
# callgrind, one thread, of scorecard binaries on corpora: total, the rows stage (inclusive),
# and in it the exclusive cost of the lexer and of RowStates' functions
S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
cd $S/c4
vs=$1; shift
for c in "$@"; do
  for v in $vs; do RAYON_NUM_THREADS=1 valgrind --tool=callgrind --callgrind-out-file=cg_${v}_$c.out $S/c4/bin/scorecard_$v $c --runs 1 > out_${v}_$c.json 2> cg_${v}_$c.log & done; wait
  for v in $vs; do
    total=$(grep -o 'Collected : [0-9]*' cg_${v}_$c.log | grep -o '[0-9]*')
    rows=$(callgrind_annotate --auto=no --inclusive=yes cg_${v}_$c.out 2>/dev/null | grep -E "genson_core::phase::rows" | head -1 | awk '{gsub(",","",$1); print $1}')
    excl=$(callgrind_annotate --auto=no cg_${v}_$c.out 2>/dev/null)
    lex=$(echo "$excl" | grep -E "tape::Lexer|tape::read|extract::scan::plain_prefix" | awk '{gsub(",","",$1); s+=$1} END {print s+0}')
    intern=$(echo "$excl" | grep -E "interned::RowStates|interned::States|interned::number_kind" | awk '{gsub(",","",$1); s+=$1} END {print s+0}')
    norm=$(callgrind_annotate --auto=no --inclusive=yes cg_${v}_$c.out 2>/dev/null | grep -E "genson_core::phase::normalise" | head -1 | awk '{gsub(",","",$1); print $1}')
    hash=$(grep -o '"schema_hash":"[0-9a-f]*"' out_${v}_$c.json)
    echo "$c $v total $total rows $rows normalise ${norm:-0} lex $lex intern $intern $hash"
  done
  rm -f cg_*_$c.out
done
