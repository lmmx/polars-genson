"""One timed run of one benchmark workload, in whichever environment runs this script.

Prints a JSON object of the operation's wall time and the user and system CPU time of all
threads, in seconds, measured around the operation only (imports and loading the input are
left out), and the process's peak resident memory in MB.

Usage: python workloads.py <workload> <data_dir>
       python workloads.py flat <data_dir>   (writes the flat workload's input)

Workloads (inputs are made by `compare.py`, see `prepare`):
- labels_infer: Avro schema inference over 30,000 rows of Wikidata labels maps
- labels_normalise: normalising those rows into typed columns (`normalise_json`)
- claims_infer: Avro schema inference over 200 rows of Wikidata-like claims
- claims_parquet: typed Parquet output of those rows (`normalise_from_parquet`)
- flat_parquet: typed Parquet output of 2,000,000 flat rows of four scalar fields, which
  the writer splits into row groups
"""

import json
import resource
import sys
import tempfile
import time
from pathlib import Path

LABELS_REPEAT = 30
CLAIMS_ROWS = 200
FLAT_ROWS = 2_000_000
# wikidata-pq's options for claims (src/wikidata/process.py), which every 1.x release takes
CLAIMS_OPTIONS = {
    "ndjson": True,
    "map_threshold": 0,
    "unify_maps": True,
    "force_field_types": {"mainsnak": "record"},
    "force_scalar_promotion": {
        "mainsnak",
        "datavalue",
        "precision",
        "latitude",
        "longitude",
    },
    "no_unify": {"qualifiers"},
}


def usage() -> tuple[float, float]:
    r = resource.getrusage(resource.RUSAGE_SELF)
    return r.ru_utime, r.ru_stime


def run(workload: str, data: Path) -> dict:
    import polars as pl
    import polars_genson

    if workload.startswith("labels"):
        df = pl.read_parquet(data / "labels.parquet")
        df = pl.concat([df] * LABELS_REPEAT)
        op = {
            "labels_infer": lambda: df.genson.infer_json_schema(
                "labels", wrap_root="labels", avro=True
            ),
            "labels_normalise": lambda: df.genson.normalise_json(
                "labels", wrap_root="labels"
            ),
        }[workload]
    elif workload == "flat_parquet":
        out = Path(tempfile.mkdtemp()) / "out.parquet"
        flat = data / f"flat_{FLAT_ROWS}.parquet"

        def op():
            polars_genson.normalise_from_parquet(
                flat, column="row", output_path=out, typed=True, ndjson=True
            )
    else:
        claims = data / f"claims_{CLAIMS_ROWS}.parquet"
        out = Path(tempfile.mkdtemp()) / "out.parquet"
        op = {
            "claims_infer": lambda: polars_genson.infer_from_parquet(
                claims, column="claims", avro=True, **CLAIMS_OPTIONS
            ),
            "claims_parquet": lambda: polars_genson.normalise_from_parquet(
                claims, column="claims", output_path=out, typed=True, **CLAIMS_OPTIONS
            ),
        }[workload]
    (user, system), t = usage(), time.perf_counter()
    op()
    wall = time.perf_counter() - t
    user1, system1 = usage()
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
    return {
        "wall": wall,
        "user": user1 - user,
        "sys": system1 - system,
        "peak_mb": peak,
    }


def write_flat(data: Path) -> None:
    """`FLAT_ROWS` JSON objects of an id, a label, a count and a score, one per row."""
    import polars as pl

    i = pl.int_range(FLAT_ROWS)
    row = pl.struct(
        id=pl.concat_str(pl.lit("Q"), i),
        label=pl.concat_str(pl.lit("item "), (i * 7919) % 1_000_003),
        count=i % 1000,
        score=i / 7,
    )
    df = pl.select(row=row.struct.json_encode())
    df.write_parquet(data / f"flat_{FLAT_ROWS}.parquet")


WORKLOADS = [
    "labels_infer",
    "labels_normalise",
    "claims_infer",
    "claims_parquet",
    "flat_parquet",
]

if __name__ == "__main__":
    if sys.argv[1] == "flat":
        write_flat(Path(sys.argv[2]))
    else:
        print(json.dumps(run(sys.argv[1], Path(sys.argv[2]))))
