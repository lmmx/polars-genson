"""Configuration for pytest tests."""

import polars as pl

# Configure Polars for tests
cfg = pl.Config()
cfg.set_tbl_cols(-1)
cfg.set_tbl_rows(-1)
