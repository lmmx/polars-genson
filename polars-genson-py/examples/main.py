#!/usr/bin/env python3
"""Simple demo for memory profiling."""

import time
from pathlib import Path

import polars as pl
import polars_genson


def main():
    """Demo for memory profiling."""
    # Hardcoded path to your fixture file
    path = (
        Path.home() / "dev/polars-genson/genson-cli/tests/data/claims_fixture_x30.jsonl"
    )
    n_rows = 400

    print(f"\nLoading first {n_rows} JSON rows from {path}")
    with open(path, "r") as f:
        json_lines = [line.strip() for line in f if line.strip()][:n_rows]

    df = pl.DataFrame({"claims": json_lines})
    print(f"DataFrame loaded with {df.height} rows")

    print("\n=== Running genson.normalise_json ===")
    t0 = time.perf_counter()

    result = df.genson.normalise_json(
        "claims",
        wrap_root="claims",
        map_threshold=0,
        unify_maps=True,
        force_field_types={"mainsnak": "record"},
        no_unify={"qualifiers"},
        decode=True,
        profile=True,
    )

    t1 = time.perf_counter()
    print(f"\nCompleted in {t1 - t0:.2f} s")
    print(f"Result type: {type(result)}")

    if hasattr(result, "shape"):
        print(f"Result shape: {result.shape}")


if __name__ == "__main__":
    main()
