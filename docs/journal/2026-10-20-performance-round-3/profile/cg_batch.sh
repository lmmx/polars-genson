#!/bin/bash
# callgrind, one thread, of prof3 sub-steps (toggle on prof3::measured)
R=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad/research/profile; S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
cg() { # name chunk preset mode
  THREADS=1 valgrind --tool=callgrind --toggle-collect='prof3::measured*' --callgrind-out-file=$R/cg/p3_$1.out $R/bin/prof3 $2 $3 $4 1 > $R/cg/p3_$1.json 2> $R/cg/p3_$1.log
  echo "$1 $?"
}
cg rowsw_sitelinks_infer $S/prod/rowsw.parquet sitelinks infer &
cg rowsw_sitelinksU_typed $S/prod/rowsw.parquet sitelinks_unify typed &
cg rowsw_sitelinks_json $S/prod/rowsw.parquet sitelinks json &
wait
cg rowsw_claims_full $S/prod/rowsw.parquet claims full &
cg c0_claims_full $S/release/chunk_0.parquet claims full &
cg rowsw_labels_typed $S/prod/rowsw.parquet labels typed &
wait
cg rowsw_descriptions_typed $S/prod/rowsw.parquet descriptions typed &
cg rowsw_aliases_typed $S/prod/rowsw.parquet aliases typed &
cg c0_sitelinks_json $S/release/chunk_0.parquet sitelinks json &
cg rowsw_sitelinks_typed $S/prod/rowsw.parquet sitelinks typed &
wait
