# parquet-rs 60 float columns unreadable by Polars before 1.43.2

## Current State

- genson-core writes all Parquet output (string, typed and lookup table) with the `arrow`/`parquet` crates, which moved from 53 to 60 in #197 (genson-core/Cargo.toml).
- `parquet` 60.0.0 sets the column order of every `FLOAT`/`DOUBLE` column to `IEEE_754_TOTAL_ORDER` in `ColumnOrder::column_order_for_type` (parquet-60.0.0/src/basic.rs:1052-1060), with no writer property to override it — `parquet` 54.3.1, 55.2.0, 56.2.1, 57.3.1, 58.0.0, 58.4.0, 59.0.0 and 59.3.0 have no `IEEE_754_TOTAL_ORDER`.
- A raw Thrift walk of the footers of `chunk_2-00114`'s typed claims output written by `parquet` 53.4.1 and by 60.0.0 finds six field paths only in the 60 file: `ColumnMetaData.encoding_stats` (all 75 columns), `Statistics` field 9 (6 columns), and `FileMetaData.column_orders` entries using union field 2 (6 columns) — the 6 are the float columns.
- Polars 1.34.0 rejects any `parquet` 60 file with a float column with `ComputeError: parquet: File out of specification: Invalid thrift: bad data`; a typed file with one `Float64` column is enough, and one with only `Int64`, `String`, `Null`, nested lists or long strings reads fine.
- Polars 1.42.1 and every earlier 1.35–1.42 release tested reject the float-column file, 1.43.2, 1.44.1 and 1.44.2 read it, and 1.43.0 and 1.43.1 are yanked on PyPI.
- pyarrow reads the same files, and parqeye read them after an upgrade and not before.
- Wikidata claims typed output has float columns (`precision__number`, `latitude__number`, `longitude__number`), so every claims file from `normalise_from_parquet(typed=True)` on `parquet` 60 is affected, whatever the chunk size — the unreadable `chunk_0-00004` outputs from the `extract_lookup` benchmark were read with an environment holding Polars older than 1.43.2.
- polars-genson 0.8.0 and 0.8.1 write with `parquet` 53.4.1, and Polars 1.34.0 reads their typed output of the same chunk.
- Polars 1.43.2 requires Python >= 3.10, and Polars ships older-CPU builds as the `polars[rtcompat]` extra (`polars-runtime-compat`) — `polars-lts-cpu` stopped at 1.33.1.
- On `fix/polars-floor`, polars-genson requires `polars>=1.43.2` (the `polars` extra and the `dev` group), replaces the `polars-lts-cpu` extra with `rtcompat = ["polars[rtcompat]>=1.43.2"]`, and requires Python >= 3.10.
- `test_typed_output_with_float_column_is_readable` (polars-genson-py/tests/parquet_test.py) fails on Polars 1.34.0 with the Invalid thrift error and passes on 1.44.2.
