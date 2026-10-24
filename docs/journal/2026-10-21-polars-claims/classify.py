"""Classify each busy thread's stack in gdb samples (sample.sh) by where the Dremel level
iterator was called from: write_rep_and_def's rep or def levels, or a leaf's array_to_page
directly (the num_values count). Usage: python classify.py samples.txt"""
import re, sys
from collections import Counter
text = open(sys.argv[1]).read()
WAIT = ("futex", "epoll", "poll", "nanosleep", "sched_yield", "pthread_cond", "__lll_lock", "syscall")
cat, n = Counter(), 0
for s in text.split("=== SAMPLE")[1:]:
    for th in re.split(r"\nThread \d+ ", s)[1:]:
        fr = re.findall(r"^#\d+\s+(?:0x[0-9a-f]+ in )?(.+)$", th, re.M)
        if not fr or any(w in fr[0] for w in WAIT):
            continue
        n += 1
        j = " | ".join(fr)
        if "BufferedDremelIter" in j:
            if "write_rep_levels" in j:
                c = "BufferedDremelIter under write_rep_and_def: rep levels"
            elif "write_def_levels" in j:
                c = "BufferedDremelIter under write_rep_and_def: def levels"
            elif "write_rep_and_def" in j:
                c = "BufferedDremelIter under write_rep_and_def: other"
            else:
                k = next(i for i, f in enumerate(fr) if "BufferedDremelIter" in f)
                caller = next((f for f in fr[k + 1:] if "BufferedDremelIter" not in f), "?")
                c = "BufferedDremelIter called from " + re.sub(r" \(.*", "", caller).split(" from ")[0][:90]
        elif "write_rep_and_def" in j:
            c = "write_rep_and_def, not in the iterator"
        elif "ZSTD" in j:
            c = "zstd"
        elif "cast" in j:
            c = "cast to Arrow (utf8view to utf8)"
        else:
            c = "other: " + re.sub(r" \(.*", "", fr[0])[:80]
        cat[c] += 1
print("busy-thread samples:", n)
for c, k in cat.most_common():
    print(f"{k:3} {k / n:5.0%}  {c}")
