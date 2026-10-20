#!/bin/bash
# prof3.rs is ../profile/prof3.rs; counters.py (here) adds the allocation counters it reads to phase.rs for the alloc-stats build, reverted after
# prof3 (measurement only) for the parent (wt-fmA, 5d22498) and the child (wt-c3), release and alloc-stats builds
S=/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad
set -e
# Each side in a target directory of its own: built into one shared directory, the child's
# copied example was the parent's (no change measured; caught by a string only the change has)
for side in A:$S/wt-fmA:target-fm B:$S/wt-c3:target-fm-b; do
  IFS=: read name tree target <<< "$side"
  cp $S/research/profile/prof3.rs $tree/genson-core/examples/prof3.rs
  (cd $tree && CARGO_TARGET_DIR=$S/$target CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none cargo build -q --release -p genson-core --example prof3 --features avro,parquet)
  cp $S/$target/release/examples/prof3 $S/fm/bin/prof3_$name
  python3 $S/fm/counters.py $tree
  (cd $tree && CARGO_TARGET_DIR=$S/$target cargo build -q --release -p genson-core --example prof3 --features avro,parquet,alloc-stats)
  git -C $tree checkout -- genson-core/src/phase.rs
  cp $S/$target/release/examples/prof3 $S/fm/bin/prof3_alloc_$name
  rm $tree/genson-core/examples/prof3.rs
  echo built $name
done
