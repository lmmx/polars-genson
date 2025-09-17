"""Helper script to generate claims parquet data for tests."""

import json
from pathlib import Path

import polars as pl

source = Path("claims_fixture_x4.parquet")
dest_jsonl = Path("claims_fixture_x4.jsonl")

if not dest_jsonl.exists():
    df = pl.read_parquet(source)
    rows = df.get_column("claims").to_list()
    dest_jsonl.write_text("\n".join(rows) + "\n")
