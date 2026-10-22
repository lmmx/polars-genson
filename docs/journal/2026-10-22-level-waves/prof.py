"""master's process() on one routed release chunk, once, in this fresh process (as
pool.py runs a chunk), unchanged except that every genson normalise_from_parquet call
gets profile=True (genson then prints its stage timings; the claims call has it already).
Each step below is wrapped: wall, CPU (all threads), minor page faults, and peak RSS
over the step's starting RSS. Rust's stdout (the stage timings) is captured per step.
Prints JSON. Usage: python prof.py <routed chunk.parquet> <workdir> <src dir> <out.json>"""
import os, sys, time
t_start = time.perf_counter()
from pathlib import Path
chunk, work, src, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), str(Path(sys.argv[3]).resolve()), Path(sys.argv[4]).resolve()
os.environ["WIKIDATA_RELEASE"] = "20261012"
import shutil
shutil.rmtree(work, ignore_errors=True); work.mkdir(parents=True); os.chdir(work)
sys.path.insert(0, src)
import functools, inspect, json, resource, re, tempfile
import polars as pl
import polars_genson  # noqa: F401
from wikidata import process as P
assert P.__file__.startswith(src), P.__file__
t_import = time.perf_counter() - t_start

def ru():
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime + r.ru_stime, r.ru_minflt, r.ru_majflt, r.ru_nvcsw, r.ru_nivcsw, r.ru_stime
def kb(field):
    for line in open("/proc/self/status"):
        if line.startswith(field):
            return int(line.split()[1])

# genson writes its stage timings to fd 2: send fd 2 to a file, read what each step added
cap = tempfile.NamedTemporaryFile("w+", delete=False)
sys.stderr.flush(); saved = os.dup(2); os.dup2(cap.fileno(), 2)
def captured_since(pos):
    sys.stderr.flush(); cap.flush()
    with open(cap.name) as f:
        f.seek(pos); text = f.read()
    return text
steps, order = {}, []
def timed(name_of):
    def wrap(f):
        @functools.wraps(f)
        def inner(*a, **k):
            name = name_of(*a, **k)
            sys.stderr.flush(); pos = os.path.getsize(cap.name)
            open("/proc/self/clear_refs", "w").write("5")
            base, t, (c, mf, jf, vc, ic, sy) = kb("VmRSS"), time.perf_counter(), ru()
            try:
                return f(*a, **k)
            finally:
                c2, mf2, jf2, vc2, ic2, sy2 = ru()
                e = steps.setdefault(name, {"calls": 0, "wall": 0.0, "cpu": 0.0, "sys": 0.0, "minflt": 0, "peak_over_base_mb": 0.0, "stages": {}})
                e["start"] = t; e["calls"] += 1; e["wall"] += time.perf_counter() - t; e["cpu"] += c2 - c; e["sys"] += sy2 - sy; e["minflt"] += mf2 - mf
                e["peak_over_base_mb"] = max(e["peak_over_base_mb"], (kb("VmHWM") - base) / 1024)
                e["stages_seq"] = []
                for m in re.finditer(r"\[profile\] ([^:]+): ([0-9.]+)s", captured_since(pos)):
                    e["stages"][m.group(1)] = e["stages"].get(m.group(1), 0.0) + float(m.group(2))
                    e["stages_seq"].append((m.group(1), float(m.group(2))))
                if name not in order: order.append(name)
        return inner
    return wrap
def caller():
    for fr in inspect.stack()[2:8]:
        if fr.filename.endswith("process.py") and "wikidata" in fr.filename:
            return fr.function
    return "?"

_nfp = P.normalise_from_parquet
P.normalise_from_parquet = timed(lambda *a, **k: f"genson.normalise_from_parquet[{k.get('column')}]")(lambda *a, **k: _nfp(*a, **{**k, "profile": True}))
P.normalise_sitelinks = timed(lambda *a, **k: "process.normalise_sitelinks (genson.normalise_json)")(P.normalise_sitelinks)
P.lookup_to_long = timed(lambda *a, **k: "process.lookup_to_long")(P.lookup_to_long)
P.atomic_sink_parquet = timed(lambda lf, dst: f"polars.sink_parquet[{Path(dst).parent.name}]")(P.atomic_sink_parquet)
P.check_ids = timed(lambda total, fr, *, table: f"process.check_ids[{table}]")(P.check_ids)
P.n_ids = timed(lambda *a, **k: f"process.n_ids[{caller()}]")(P.n_ids)
P.inferred_schema = timed(lambda *a, **k: "process.inferred_schema")(P.inferred_schema)
P.pl.read_parquet = timed(lambda *a, **k: f"polars.read_parquet[{caller()}]")(pl.read_parquet)

data, output, state = (work / d for d in ("data", "out", "state"))
for d in (data, output, state): d.mkdir(parents=True)
(data / chunk.name).symlink_to(chunk)
open("/proc/self/clear_refs", "w").write("5")
t, (c, mf, jf, vc, ic, sy) = time.perf_counter(), ru()
P.process(data, output, repo_id="unused", state_dir=state)
wall = time.perf_counter() - t
c2, mf2, jf2, vc2, ic2, sy2 = ru()
os.dup2(saved, 2)
import importlib.metadata as _m, subprocess as _sp
res = {"chunk": str(chunk), "polars": pl.__version__, "polars_genson": _m.version("polars-genson"), "pyarrow": _m.version("pyarrow"),
       "wikidata_pq_commit": _sp.run(["git", "-C", src, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip(), "import_wall": round(t_import, 3), "process_wall": round(wall, 3),
       "process_cpu": round(c2 - c, 2), "process_sys": round(sy2 - sy, 2), "minflt": mf2 - mf, "majflt": jf2 - jf, "vcsw": vc2 - vc, "ivcsw": ic2 - ic,
       "process_peak_rss_mb": kb("VmHWM") // 1024, "maxrss_mb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss // 1024,
       "order": order, "steps": steps,
       "outputs_mb": {p.parent.name: round(p.stat().st_size / 1e6, 2) for p in output.rglob("*.parquet")}}
out.write_text(json.dumps(res, indent=1))
print(f"{chunk.parent.parent.name}/{chunk.name}: process {wall:.2f}s wall, {c2 - c:.1f}s cpu, peak {res['process_peak_rss_mb']} MB")
