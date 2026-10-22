"""gdb samples -> busy thread samples per stage, and within a stage per owning function: the
innermost frame in parquet/arrow/genson/polars-genson source (std, alloc, core, libc, mimalloc
frames are charged to it). Also the share of each owner's samples whose innermost frame is the
allocator or memcpy. Usage: owner.py <stage> <topN> <samples...>  (stage 'all' for stage table)"""
import re, sys, collections

STAGES = [("write", ("write_struct_columns", "write_batches", "open_writer", "unrepeated_leaves", "write_pruned")),
          ("conform", ("conform::conform",)), ("decode", ("arrow_plan", "ArrowPlan", "normalise_row", "values_to_struct_array", "decode")),
          ("infer", ("infer", "schema_from", "SchemaBuilder", "unify")), ("extract", ("extract_invariants", "invariant")),
          ("read", ("read_string_column", "read_columns", "ParquetRecordBatchReader"))]
FR = re.compile(r"#\d+\s+(?:0x[0-9a-f]+ in )?(\S.*?) \((.*?)\)(?: at (\S+):(\d+)| from (\S+))?$")
def frames(th):
    out = []
    for line in th.split("\n"):
        if not line.startswith("#"): continue
        m = FR.match(line)
        if m: out.append((m.group(1), m.group(3) or m.group(5) or "", m.group(4) or ""))
        else:
            m2 = re.match(r"#\d+\s+(?:0x[0-9a-f]+ in )?(\S+)", line); out.append((m2.group(1) if m2 else "?", "", ""))
    return out
def is_low(fn, path):
    return (path.startswith("/rustc/") or "mimalloc" in path or "libc" in path or "nptl" in path or "sysdeps" in path
            or fn.startswith(("mi_", "_mi_", "__memcpy", "__memmove", "__memset", "memcpy", "realloc", "malloc", "free", "__rust_", "alloc::"))
            or "/hashbrown-" in path or "/rayon-core-" in path or "/rayon-" in path or "/crossbeam" in path or "/ahash-" in path)
def is_alloc(fn, path):
    return fn.startswith(("mi_", "_mi_", "__memcpy", "__memmove", "__memset", "memcpy", "realloc", "malloc", "__rust_")) or "mimalloc" in path or "raw_vec" in path
def short(path):
    for k in ("parquet-60.0.0/", "arrow-", "genson-core/", "polars-genson-py/", "zstd", "simd-json", "serde_json", "sonic"):
        if k in path: return path.split("index.crates.io-1949cf8c6b5b557f/")[-1]
    return path
