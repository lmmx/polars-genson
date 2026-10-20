"""scripts/scorecard.py's `measure` for master alone, without creating another git worktree:
the checkout is $S/prof3 (master d38635e) and the builds are in $S/target-prof3.
Callgrind outputs are kept in $R/cg/. Usage: python3 sc_measure.py <part> where part is
instructions | allocations | timing (writes $R/sc_<part>.json)."""
import json
import sys
from pathlib import Path

S = Path("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad")
R = S / "research/profile"
sys.path.insert(0, str(S / "prof3/scripts"))
import scorecard as sc  # noqa: E402

REL = S / "target-prof3/release/examples/scorecard"
DBG = S / "target-prof3/debug/examples/scorecard"
part = sys.argv[1]
cache = R / "cg"
cache.mkdir(exist_ok=True)
sha = "d38635e00000"
corpora = sc.run([str(REL), "--list"]).split()
out = {}
for corpus in corpora:
    print(part, corpus, file=sys.stderr, flush=True)
    if part == "instructions":
        counted, attribution = sc.instructions(REL, corpus, cache, sha)
        one = sc.workload(REL, corpus, 1)
        out[corpus] = {k: one[k] for k in ("rows", "bytes", "processed", "schema_hash")}
        out[corpus].update(instructions=counted, attribution=attribution)
    elif part == "allocations":
        out[corpus] = sc.workload(DBG, corpus, 1, threads=1)
    elif part == "timing":
        runs = [sc.workload(REL, corpus, 2) for _ in range(int(sys.argv[2]) if len(sys.argv) > 2 else 5)]
        out[corpus] = {"wall": [r["runs"][-1]["wall"] for r in runs], "cpu": [r["runs"][-1]["cpu"] for r in runs],
                       "peak_rss_kb": [r["peak_rss_kb"] for r in runs], "bytes": runs[0]["bytes"]}
(R / f"sc_{part}.json").write_text(json.dumps(out, indent=1))
