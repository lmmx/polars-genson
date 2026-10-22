"""Write-stage busy samples bucketed by owning operation (owner from owner.py's rule; allocator
time charged to its owner). Usage: buckets.py <samples...>"""
import re, sys, collections, importlib.util
spec = importlib.util.spec_from_file_location("o", __file__.rsplit('/',1)[0] + "/owner_lib.py"); o = importlib.util.module_from_spec(spec); spec.loader.exec_module(o)
def bucket(fn, path, line, fr):
    f = path.rsplit("/", 1)[-1]; ln = int(line or 0)
    if fn.startswith(("ZSTD_", "HUF_", "FSE_", "MEM_")) or "zstd/lib" in path or "zstd-safe" in path or "/zstd-0" in path: return "zstd compression (C)"
    if "compression.rs" in path: return "compress output buffer (reserve compress_bound)"
    if "byte_array.rs" in path and 340 <= ln <= 365 and fn.startswith("encode"):
        return "dict: indices Vec<u64> growth" if ln == 355 else "dict: intern + push"
    if "interner.rs" in path: return "dict: intern + push"
    if "dict_encoder.rs" in path: return "dict (fixed-width): intern + push"
    if "byte_array.rs" in path and ln < 340: return "plain/fallback byte-array encoding"
    if "byte_array.rs" in path and "compute_min_max" in fn: return "statistics (min/max)"
    if "byte_array.rs" in path: return "byte_array other (flush, gather)"
    if "arrow_writer/levels.rs" in path:
        if "append_run" in fn or "extend_uniform" in fn or "append_rep" in fn or "append_def" in fn: return "levels: append_run (rep/def runs)"
        if "materialize" in fn or "extend_from_iter" in fn: return "levels: materialize / extend"
        if "write_leaf" in fn: return "levels: write_leaf (non_null_indices, leaf levels)"
        return "levels: list/struct traversal"
    if "encodings/rle.rs" in path or "bit_util.rs" in path or "encodings/levels.rs" in path: return "level/index RLE + bit-packing"
    if "metadata/mod.rs" in path or "page_index" in path or "statistics" in path: return "level histograms / stats metadata"
    if "column/writer/mod.rs" in path:
        if "add_data_page" in fn or "write_dictionary_page" in fn: return "page assembly buffers (add_data_page)"
        return "column writer other"
    if "encoder.rs" in path or "encoding" in path: return "fixed-width value encoding"
    if "genson-core" in path: return "genson write_batches / sampling"
    if "bit_iterator" in path or "bit_chunk" in path or "arrow-buffer" in path or "arrow-array" in path or "arrow-data" in path: return "arrow buffer/bitmap iteration"
    return f"other ({f})"
for label, paths in (("c0", [p for p in sys.argv[1:] if "c0_" in p]), ("s90", [p for p in sys.argv[1:] if "s90_" in p])):
    b = collections.Counter(); ba = collections.Counter(); n = 0
    for p in paths:
        for s in open(p).read().split("=== SAMPLE")[1:]:
            for th in re.split(r"\nThread \d+ ", s)[1:]:
                if "genson-core/src" not in th: continue
                fr = o.frames(th)
                if not fr: continue
                joined = " ".join(f for f, _, _ in fr); head = " ".join(f for f, _, _ in fr[:6])
                if "futex_wait" in head or "no_work_found" in head: continue
                if not any(k in joined for k in ("write_struct_columns", "write_batches", "open_writer", "unrepeated_leaves")): continue
                i = next((i for i, (f, p_, _) in enumerate(fr) if not o.is_low(f, p_)), 0)
                k = bucket(*fr[i], fr); b[k] += 1; ba[k] += o.is_alloc(*fr[0][:2]); n += 1
    print(f"\n{label}: {n} busy write samples (alloc% = samples whose innermost frame is allocator/memcpy)")
    for k, v in b.most_common(): print(f"  {100*v/n:5.1f}%  alloc {100*ba[k]/v:3.0f}%  {k}")
