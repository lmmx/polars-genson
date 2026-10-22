"""wikidata-pq's normalise_claims_direct (the claims step of process(), unchanged) on one
routed release chunk, `runs` times in this process, each into fresh paths; prints genson's
stage timings and each run's wall and CPU. Usage: claims_step.py <chunk> <workdir> <src> <runs>"""
import os, resource, shutil, sys, time
from pathlib import Path
chunk, work, src, runs = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), str(Path(sys.argv[3]).resolve()), int(sys.argv[4])
os.environ["WIKIDATA_RELEASE"] = "20261012"
shutil.rmtree(work, ignore_errors=True); work.mkdir(parents=True); os.chdir(work)
sys.path.insert(0, src)
from wikidata import process as P
assert P.__file__.startswith(src)
def cpu():
    r = resource.getrusage(resource.RUSAGE_SELF); return r.ru_utime + r.ru_stime
print("READY", os.getpid(), flush=True)
for i in range(runs):
    d = work / f"run{i}"; d.mkdir()
    t, c = time.perf_counter(), cpu()
    P.normalise_claims_direct(chunk, d / "claims.parquet", d / "lookup.parquet", d / "quarantine.parquet")
    print(f"RUN {i}: {time.perf_counter() - t:.2f}s wall, {cpu() - c:.1f}s cpu", file=sys.stderr, flush=True)
