#!/usr/bin/env bash
# Build a measurement copy of genson-core's bench_claims in its own worktree and target dir.
#
# Usage: build_harness.sh <scratch dir> <name> <git rev> [--no-count] [prototype.py ...]
#
# Creates <scratch>/wt-<name> at <rev>, applies acct_patch.py (with --no-count if given,
# so the timings carry no allocation-counting overhead), then each prototype script (each
# takes the checkout path as its argument), builds bench_claims with its own target dir
# (two worktrees sharing one target dir gave one binary for both) and copies it to
# <scratch>/bin/<name>.
set -euo pipefail
S="$1"; NAME="$2"; REV="$3"; shift 3
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(git -C "$HERE" rev-parse --show-toplevel)"
WT="$S/wt-$NAME"
NOCOUNT=""
if [ "${1:-}" = "--no-count" ]; then NOCOUNT="--no-count"; shift; fi
rm -rf "$WT"; git -C "$REPO" worktree prune
git -C "$REPO" worktree add -q --detach "$WT" "$REV"
python3 -I "$HERE/acct_patch.py" "$WT" $NOCOUNT >/dev/null
for proto in "$@"; do python3 -I "$proto" "$WT"; done
(cd "$WT" && CARGO_TARGET_DIR="$S/target-$NAME" cargo build -q --release -p genson-core --example bench_claims 2>&1 | grep -E "^error" -A12 || true)
mkdir -p "$S/bin"
cp "$S/target-$NAME/release/examples/bench_claims" "$S/bin/$NAME"
echo "built $S/bin/$NAME ($(git -C "$WT" rev-parse --short HEAD) $NOCOUNT $*)"
