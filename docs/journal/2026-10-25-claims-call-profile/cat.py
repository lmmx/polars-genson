"""Categorise perf script samples (stdin) of the claims call: user vs kernel, and stage."""
import sys, re, collections
cnt = collections.Counter(); tot = 0
def cat(frames):
    leaf = frames[0]
    s = " ".join(frames)
    kernel = leaf.endswith("[k]") or "[kernel" in leaf
    if "extract_invariants" in s or "extract7" in s: stage = "extract"
    elif "conform" in s: stage = "conform"
    elif "read_string_column" in s or "read_columns" in s or "parquet" in s.lower() and "ArrowPlan" not in s: stage = "read"
    elif "ArrowPlan" in s or "normalise5typed" in s:
        if "unescape" in s: stage = "decode: unescape"
        elif "Lexer" in s or "4tape3lex" in s or "tape3lex" in s: stage = "decode: lex"
        elif re.search(r"append_value|append_null|append_option|append_n|push_offset|BufferBuilder|finish_grow|raw_vec|grow_one|reserve|Column12append|Column6finish|Column3pad|memmove|memcpy|memset|mi_|_mi_|realloc|malloc|free", leaf) or kernel: stage = "decode: pushes/alloc"
        else: stage = "decode: walk"
    elif "write_lookup" in s: stage = "lookup"
    else: stage = "other"
    return ("kernel" if kernel else "user"), stage
frames = []
for line in sys.stdin:
    line = line.rstrip("\n")
    if not line.strip():
        if frames:
            k, st = cat(frames); cnt[(k, st)] += 1; tot += 1
        frames = []; continue
    if line.startswith("\t") or line.startswith(" "):
        parts = line.split()
        if len(parts) >= 2:
            sym = parts[1]; dso = parts[-1]
            frames.append(sym + (" [k]" if "kernel" in dso else ""))
    else:
        frames = []
if frames:
    k, st = cat(frames); cnt[(k, st)] += 1; tot += 1
user = sum(v for (k, _), v in cnt.items() if k == "user")
print(f"samples {tot}, user {user} ({100*user/tot:.0f}%)")
for kind in ("user", "kernel"):
    for (k, st), v in sorted(cnt.items(), key=lambda x: -x[1]):
        if k == kind: print(f"  {kind:6} {st:22} {100*v/tot:5.1f}% of all  {100*v/(user if kind=='user' else tot-user):5.1f}% of {kind}")
