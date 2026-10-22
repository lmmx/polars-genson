"""Samples from gdbsampler.py -> per genson stage, the share of busy thread samples; within
the Parquet write, by sub-stage. A thread is busy unless its innermost frames are a futex
wait or rayon's sleep. Stage: by the frames on the stack (write_struct_columns /
write_batches -> write). Write sub-stage: the innermost frame that matches, in this order:
allocator (mi_*, realloc, memcpy under them), zstd, statistics, dictionary hashing, level
computation, level RLE encoding, value encoding, page building, dictionary sampling,
unnest, other. Usage: classify.py <samples file> ..."""
import re, sys, collections
STAGES = [("write", ("write_struct_columns", "write_batches", "open_writer", "unrepeated_leaves", "write_pruned")),
          ("conform", ("conform::conform",)), ("decode", ("arrow_plan", "ArrowPlan", "normalise_row", "values_to_struct_array", "decode")),
          ("infer", ("infer", "schema_from", "SchemaBuilder", "unify")), ("extract", ("extract_invariants", "invariant")),
          ("read", ("read_string_column", "read_columns", "ParquetRecordBatchReader"))]
SUB = [("allocator: realloc/memcpy/fresh pages", lambda f: f.startswith(("mi_", "_mi_", "realloc", "__rust_realloc", "__rust_alloc", "alloc::alloc"))),
       ("compression (zstd)", lambda f: f.startswith(("ZSTD_", "HUF_", "MEM_", "FSE_", "zstd"))),
       ("statistics (min/max)", lambda f: "compute_min_max" in f or f.startswith(("partial_cmp", "lt<", "cmp<")) or "Statistics" in f or "update_stats" in f),
       ("dictionary (interner/hash)", lambda f: "Interner" in f or "find_or_find_insert" in f or "DictEncoder" in f or "dict_encoder" in f),
       ("level computation (compute_leaves)", lambda f: "visit_leaves" in f or "write_list" in f or "compute_leaves" in f or "levels::" in f or "LevelInfoBuilder" in f or "append_rep_level" in f or "write_struct" in f),
       ("level encoding (RLE/bit-pack)", lambda f: "put_with_observer" in f or "RleEncoder" in f or "flush_buffered_values" in f or "is_accumulating_rle" in f or "extend_run" in f or "flush_bit_packed" in f or "LevelEncoder" in f),
       ("value encoding", lambda f: f.startswith(("encode", "write_gather", "put_one", "write_slice", "put<")) or "PlainEncoder" in f or "ByteArrayEncoder" in f or "write_mini_batch" in f),
       ("page building / flush", lambda f: "add_data_page" in f or "flush_data_page" in f or "write_data_page" in f or "compress" == f),
       ("dictionary sampling (xxh64)", lambda f: "unrepeated_leaves" in f or "xxh64" in f or "sample" == f),
       ("unnest", lambda f: "unnest" in f)]
def frames(th):
    """Each frame's function part (generics kept: inlined frames name their callee there)."""
    out = []
    for line in th.split("\n"):
        m = re.match(r"#\d+\s+(?:0x[0-9a-f]+ in )?(.*?)(?: \(.*\))?(?: at .*| from .*)?$", line)
        if m: out.append(m.group(1))
    return out
busy = collections.Counter(); sub = collections.Counter(); idle_w = 0; nsamp = 0; wsamp = 0
for path in sys.argv[1:]:
    for s in open(path).read().split("=== SAMPLE")[1:]:
        nsamp += 1; in_write = False
        for th in re.split(r"\nThread \d+ ", s)[1:]:
            fr = frames(th)
            if not fr: continue
            joined = " ".join(fr)
            if "genson-core/src" not in th and "polars-genson-py/src" not in th and "genson_core::" not in th: continue
            head = " ".join(fr[:6])
            idle = "futex_wait" in head or "no_work_found" in head or "wait_until_cold" in head and "futex" in head
            stage = next((n for n, keys in STAGES if any(k in joined for k in keys)), "other genson")
            if stage == "write": in_write = True
            if idle:
                if stage == "write": idle_w += 1
                continue
            busy[stage] += 1
            if stage == "write":
                names = [re.sub(r"<.*", "", f).split("::")[-1] for f in fr]
                cat = next((c for f, full in zip(names, fr) for c, test in SUB if test(f) or test(full.split("<")[0])), "other")
                sub[cat] += 1
        wsamp += in_write
tot = sum(busy.values())
print(f"{nsamp} samples, {tot} busy thread samples in genson")
for k, v in busy.most_common(): print(f"  {k:14s} {v:5d}  {100*v/tot:5.1f}%")
w = sum(sub.values())
print(f"\nwrite: {wsamp} samples during the write, {w} busy and {idle_w} idle thread samples ({w / max(wsamp,1):.2f} busy threads per sample)")
for k, v in sub.most_common(): print(f"  {k:40s} {v:5d}  {100*v/w:5.1f}%")
