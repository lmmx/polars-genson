"""Aggregate gdb samples: for each sample, the threads whose innermost frame is not a wait
(futex, epoll, poll, nanosleep, sched_yield), and for those the frames' function names.
Prints the most common innermost functions, and inclusive counts of polars functions."""
import re, sys
from collections import Counter
text = open(sys.argv[1]).read()
samples = text.split("=== SAMPLE")[1:]
WAIT = ("futex", "epoll", "poll", "nanosleep", "sched_yield", "pthread_cond", "__lll_lock", "syscall", "clock_nanosleep", "do_futex")
inner, incl, busy = Counter(), Counter(), []
for s in samples:
    n = 0
    for th in re.split(r"\nThread \d+ ", s)[1:]:
        frames = re.findall(r"^#\d+\s+(?:0x[0-9a-f]+ in )?(\S+?)\s*\(", th, re.M)
        if not frames or any(w in frames[0] for w in WAIT):
            continue
        n += 1
        inner[frames[0]] += 1
        for f in set(frames):
            incl[re.sub(r"::h[0-9a-f]{16}$", "", f)] += 1
    busy.append(n)
print("samples", len(samples), "busy threads per sample", busy)
print("\n-- innermost"); [print(f"{c:4} {f[:160]}") for f, c in inner.most_common(25)]
print("\n-- inclusive (polars/parquet frames)"); [print(f"{c:4} {f[:160]}") for f, c in incl.most_common(200) if "polars" in f or "parquet" in f][:45]
