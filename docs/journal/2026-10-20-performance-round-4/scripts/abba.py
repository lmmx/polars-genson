"""Interleaved ABBA timing of scorecard binaries: each process runs a warm-up and N measured
runs; the measured runs' wall and CPU, median [min-max] over rounds."""
import json, statistics as st, subprocess, sys
S = "/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad"
a, b, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
for corpus in sys.argv[4:]:
    res = {a: [], b: []}
    for r in range(rounds):
        for v in ([a, b] if r % 2 == 0 else [b, a]):
            out = subprocess.run([f"{S}/c4/bin/scorecard_{v}", corpus, "--runs", "16"], capture_output=True, text=True).stdout
            d = json.loads(out.strip().splitlines()[-1])
            res[v] += [(x["wall"], x["cpu"]) for x in d["runs"][1:]]
    line = [corpus]
    for v in (a, b):
        w = [x[0] for x in res[v]]; c = [x[1] for x in res[v]]
        line.append(f"{v} wall {st.median(w)*1000:.1f} ms [{min(w)*1000:.1f}-{max(w)*1000:.1f}] cpu {st.median(c)*1000:.1f} ms")
    wa, wb = st.median(x[0] for x in res[a]), st.median(x[0] for x in res[b])
    line.append(f"wall {100*(wb-wa)/wa:+.1f}%")
    print(" | ".join(line), flush=True)
