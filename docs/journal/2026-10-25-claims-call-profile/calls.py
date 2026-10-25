"""The four normalise_in_schema calls process() makes on a chunk (labels, descriptions,
aliases via normalise_map_direct; claims via normalise_claims_direct's call and its
read_all), each timed: wall, process CPU (user, sys), and genson's [profile] stages.
Usage: calls.py <wikidata-pq src> <chunk.parquet> <out dir> <passes>"""
import os, resource, sys, time, types, io, contextlib, re, json
from pathlib import Path
os.environ["WIKIDATA_RELEASE"] = "20261012"
src, chunk, out, passes = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4])
sys.modules["huggingface_hub"] = types.SimpleNamespace(HfFileSystem=object)
sys.modules["wikidata.pull"] = types.SimpleNamespace(_hf_dl_subdir=None)
sys.path.insert(0, src)
import wikidata.process as P
import pyarrow as pa
out.mkdir(parents=True, exist_ok=True)
ru = lambda: resource.getrusage(resource.RUSAGE_SELF)

def claims():
    stream = P.normalise_in_schema("claims", chunk, column="claims", output_path=None, output_format="arrow",
        output_column="claims", wrap_root="claims", **P.CLAIMS_INFERENCE_OPTIONS, profile=True, max_builders=100,
        typed=True, keep_columns=["id"], extract_invariants=P.LABEL_INVARIANTS, lookup_output_path=out / "lookup.parquet",
        prune=P.QUARANTINE_FIELDS, prune_output_path=out / "pruned.parquet", avro_schema=P.CLAIMS_AVRO,
        map_encoding="kv", schema=P.claims_schema, unnest=True)
    return pa.RecordBatchReader.from_stream(stream).read_all()

calls = {
    "labels": lambda: P.normalise_map_direct(chunk, out / "labels.parquet", key="labels"),
    "descriptions": lambda: P.normalise_map_direct(chunk, out / "descriptions.parquet", key="descriptions"),
    "aliases": lambda: P.normalise_map_direct(chunk, out / "aliases.parquet", key="aliases", lists=True),
    "claims": claims,
}
for p in range(passes):
    for name, fn in calls.items():
        # genson's [profile] lines go to the process's stderr (fd 2)
        r, w = os.pipe(); saved = os.dup(2); os.dup2(w, 2); os.close(w)
        r0, t = ru(), time.perf_counter()
        with contextlib.redirect_stdout(io.StringIO()):
            res = fn()
        wall, r1 = time.perf_counter() - t, ru()
        os.dup2(saved, 2); os.close(saved)
        err = os.read(r, 1 << 20).decode(); os.close(r)
        del res
        stages = {m[1]: float(m[2]) for m in re.finditer(r"\[profile\] (.+?): ([\d.]+)s", err)}
        if p == passes - 1 or True:
            print(json.dumps({"chunk": chunk.stem, "pass": p, "call": name, "wall": round(wall, 3),
                "user": round(r1.ru_utime - r0.ru_utime, 3), "sys": round(r1.ru_stime - r0.ru_stime, 3),
                "stages": stages}), flush=True)
