"""Helper script to generate claims parquet data for tests."""

import json
from pathlib import Path

import polars as pl

source = Path("claims_fixture.json")
dest_pq = Path("claims_fixture.parquet")

if not dest_pq.exists():
    json_data = json.dumps(json.loads(source.read_text()))
    pl.DataFrame({"claims": [json_data]}).write_parquet(dest_pq)
