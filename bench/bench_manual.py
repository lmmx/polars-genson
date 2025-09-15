# bench_manual.py
import time
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401

LABELS = Path(__file__).parent / "data" / "labels.parquet"
DATA_FULL = pl.read_parquet(LABELS)

DATA_SMALL = DATA_FULL.head(10)
DATA_MEDIUM = DATA_FULL.head(100)


def bench(name: str, df: pl.DataFrame, decode: bool):
    t0 = time.perf_counter()
    _ = df.genson.normalise_json("labels", wrap_root="labels", decode=decode)
    t1 = time.perf_counter()

    print(f"{name} | decode={decode} | {(t1 - t0):.3f} s")


if __name__ == "__main__":
    for name, dataset in [
        ("small", DATA_SMALL),
        ("medium", DATA_MEDIUM),
        ("large", DATA_FULL),
    ]:
        bench(name=name, df=dataset, decode=False)
        bench(name=name, df=dataset, decode=True)
