"""Runs e2e_one.py: per chunk and step, one warm-up process then N measured processes (each
running the step `reps` times in-process), steps interleaved round-robin so that load
drift on the shared machine spreads over all of them. Appends JSON lines to e2e_runs.jsonl
with the [profile] stage lines polars-genson printed (claims passes profile=True).

Usage: python3 e2e_drive.py <runs> <reps> chunk:release[,chunk:release] step[,step]
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

S = Path("/tmp/claude-0/-home-user-polars-genson/a3c6f4e5-fac5-5911-974c-cd24e5aa55fb/scratchpad")
R = S / "research/profile"
PY = S / "pypi104/env/bin/python"
runs, reps = int(sys.argv[1]), int(sys.argv[2])
chunks = [c.split(":") for c in sys.argv[3].split(",")]
steps = sys.argv[4].split(",")
(R / "tmp").mkdir(exist_ok=True)
with open(R / "e2e_runs.jsonl", "a") as log:
    for run in range(-1, runs):
        for chunk, release in chunks:
            for step in steps:
                work = Path(tempfile.mkdtemp(dir=R / "tmp"))
                p = subprocess.run([str(PY), "-I", str(R / "e2e_one.py"), chunk, step, str(work), release, str(reps)],
                                   capture_output=True, text=True)
                shutil.rmtree(work)
                if p.returncode:
                    sys.exit(p.stderr[-3000:])
                out = json.loads(p.stdout.strip().splitlines()[-1])
                prof = [(m.group(1), float(m.group(2))) for m in re.finditer(r"\[profile\] ([\w+ ]+): ([0-9.]+)s", p.stderr)]
                out.update(run=run, release=release, profile=prof)
                log.write(json.dumps(out) + "\n")
                log.flush()
                print(run, Path(chunk).name, step, f"{out['runs'][-1]['wall']:.3f}", flush=True)
