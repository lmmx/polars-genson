"""wikidata-pq's process() (unchanged) on each chunk in turn, all in this one process, as a
pool that reuses its worker would: per chunk its wall, user and system CPU, minor faults, and
the RSS left after it. Usage: reuse.py <chunks file> <workdir> <src>"""
import os, resource, shutil, sys, time
from pathlib import Path
chunks = [l.split()[1] for l in open(sys.argv[1])]
work, src = Path(sys.argv[2]).resolve(), str(Path(sys.argv[3]).resolve())
os.environ["WIKIDATA_RELEASE"] = "20261012"
sys.path.insert(0, src)
t0 = time.perf_counter()
import polars_genson  # noqa: F401
from wikidata import process as P
print(f"imports {time.perf_counter() - t0:.2f}s", flush=True)
def ru():
    r = resource.getrusage(resource.RUSAGE_SELF); return r.ru_utime, r.ru_stime, r.ru_minflt
def rss():
    return next(int(l.split()[1]) for l in open("/proc/self/status") if l.startswith("VmRSS")) // 1024
total = 0
for i, chunk in enumerate(chunks):
    d = work / f"c{i}"; shutil.rmtree(d, ignore_errors=True)
    data, out, state = d / "data", d / "out", d / "state"
    for x in (data, out, state): x.mkdir(parents=True)
    (data / Path(chunk).name).symlink_to(chunk)
    devnull = os.open(os.devnull, os.O_WRONLY); saved = os.dup(1); os.dup2(devnull, 1)
    t, (u, s, f) = time.perf_counter(), ru()
    P.process(data, out, repo_id="unused", state_dir=state)
    w = time.perf_counter() - t; u2, s2, f2 = ru(); total += w
    os.dup2(saved, 1)
    print(f"{Path(chunk).parent.parent.name}/{Path(chunk).name}: wall {w:.2f} user {u2-u:.2f} sys {s2-s:.2f} faults {(f2-f)/1e3:.0f}k rss_after {rss()} MB", flush=True)
    shutil.rmtree(d)
print(f"total process() wall {total:.2f}s")
