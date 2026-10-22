"""prof.py's process() run, plus a timeline: every 20 ms the process's user and system CPU
and minor faults (/proc/self/stat); the claims
stages are laid end to end from the call's start by genson's own durations. Per claims stage it prints wall,
user CPU, system CPU and minor faults. Usage: timeline.py <chunk> <workdir> <src> <out.json>"""
import os, sys, threading, time, json
CLK = os.sysconf("SC_CLK_TCK")
samples, lines = [], []
def stat():
    f = open("/proc/self/stat").read().rsplit(")", 1)[1].split()
    return int(f[7]), int(f[11]) / CLK, int(f[12]) / CLK  # minflt, utime, stime
# Sampled from another process: genson holds the GIL, so no thread here runs during a call
import subprocess, tempfile
smp_path = tempfile.mktemp(suffix=".samples")
smp = subprocess.Popen([sys.executable, "-I", "-c", f"""
import os, time
clk = os.sysconf("SC_CLK_TCK"); out = open({smp_path!r}, "w")
while True:
    try: f = open("/proc/{os.getpid()}/stat").read().rsplit(")", 1)[1].split()
    except OSError: break
    out.write(f"{{time.perf_counter()}} {{f[7]}} {{int(f[11]) / clk}} {{int(f[12]) / clk}}\\n"); out.flush(); time.sleep(0.02)
"""])
sys.argv = [sys.argv[0], *sys.argv[1:]]
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "prof.py")).read())
time.sleep(0.1)
smp.kill()
samples = [tuple(float(x) for x in l.split()) for l in open(smp_path) if len(l.split()) == 4]
def at(t):  # last sample at or before t
    best = samples[0]
    for s in samples:
        if s[0] <= t: best = s
        else: break
    return best
# claims stages: laid end to end from the claims call's start, by genson's own durations
cs = steps["genson.normalise_from_parquet[claims]"]
prev_t = cs["start"]
rows = []
for name, dur in cs["stages_seq"]:
    a, b = at(prev_t), at(prev_t + dur)
    rows.append({"stage": name, "wall": dur, "user": round(b[2] - a[2], 2), "sys": round(b[3] - a[3], 2), "minflt": b[1] - a[1]})
    prev_t += dur
a, b = at(prev_t), at(cs["start"] + cs["wall"])
rows.append({"stage": "(after the stages, in the call)", "wall": round(cs["start"] + cs["wall"] - prev_t, 3), "user": round(b[2] - a[2], 2), "sys": round(b[3] - a[3], 2), "minflt": b[1] - a[1]})
json.dump(rows, open(str(out) + ".stages.json", "w"), indent=1)
for x in rows:
    print(f"  {x['stage']:32s} wall {x['wall']:6.3f}  user {x['user']:5.2f}  sys {x['sys']:5.2f}  minflt {x['minflt']:,}", file=sys.__stdout__)
