# 2026-09-25 Performance work plan after 0.7.10 (#194, wikidata-pq #11)

## Current State

- polars-genson 0.7.10 (#193, borrowed column strings in the plugin) runs `bench_genson_versions.py --dev` on `chunk_0-00004-of-00546.parquet` (0.615 GB, 1818 rows, 1648.9 MB `claims`) in 6.5s with 5.0 GB peak RSS on the i9-10900K — 0.7.8 takes 10.5s and 8.1 GB, 0.7.4 takes 298s, single run each (wikidata repo `bench_genson_versions.py`)
- A separate 4-core, 15 GB session timed `normalise_claims_direct` (wikidata repo `src/wikidata/process.py:186-227`) on `chunk_5280.parquet` (10k rows, 243 MB `claims`) with the 0.7.10 wheel: `read_parquet` 2.3s, infer 6.4s, normalise 2.3s, `write_parquet` of the temp file 6.8s, `pl.read_parquet(tmp)` 0.4s, `str.json_decode(dtype=schema)` 28.8s, `sink_parquet` 3.1s, single run — none of these stage timings are reproduced on the i9
- In that session `pyarrow.json.read_json` with the explicit schema decoded the same normalised NDJSON in 2.85s and gave a frame `DataFrame.equals` to `json_decode(...).unnest()`, and `pl.read_ndjson` with the schema took 13s
- `normalise_from_parquet` reads the input column into `json_strings: Vec<String>`, infers, normalises every row to a `serde_json::Value` serialised into `normalised_strings: Vec<String>`, drops `json_strings`, and writes `normalised_strings` as a string column (polars-genson-py/src/parquet_io.rs:154-290)
- `write_string_column` copies the `Vec<String>` into a `StringArray`/`LargeStringArray` and writes one row group with default `WriterProperties` (dictionary on, statistics on, no compression) (genson-core/src/parquet.rs:126-171) — the 4-core session measured the temp file at 408 MB, 1.7× the input, and pyarrow wrote the same table in 0.37–0.53s (12.5 MB with zstd)
- `read_string_column` calls `.to_string()` on every row of each `StringArray`/`LargeStringArray` batch (genson-core/src/parquet.rs:27-110), while the core has accepted `&[S: AsRef<str>]` since #193
- `prepare_json_bytes` with `wrap_root` set runs `validate_ndjson`/`validate_json` (serde `IgnoredAny`), then `serde_json::from_str` into a `Value`, then `serde_json::to_writer` of `json!({ field: inner_val })`, before the simd-json parse (genson-core/src/schema.rs:64-75, 159-236) — `bench_claims` does not set `wrap_root`, and the 4-core session measured the summed parallel phase at 2.94s with the wrap and 2.45s without (single run)
- Each row goes through build `SchemaNode` → `to_schema()` → xxh64 of the serialised `Value` → `SchemaBuilder::add_schema_mut`, and `ListStrategy::add_object` folds/reduces arrays of `PARALLEL_PROCESSING_BOUNDARY` or more items through the same `to_schema` round trip (genson-core/src/schema.rs, genson-core/src/genson_rs/strategy/array.rs:95)
- The record branch of `normalise_value` deep-copies each field with `m.get(name).cloned()` at every nesting level (genson-core/src/normalise.rs:192)
- `rewrite_objects` (genson-core/src/schema/map_inference.rs:210) and `reorder_unions` (genson-core/src/schema.rs:84) run single-threaded after the chunk loop
- `polars-jsonschema-bridge` maps Avro and JSON Schema to Polars dtype strings (`avro_type_to_polars_type`, polars-jsonschema-bridge/src/deserialise.rs:113), and `avro_to_polars_schema` maps Avro to a `pl.Schema` in Python (polars-genson-py/python/polars_genson/__init__.py:1236)
- genson-core's `parquet` feature pulls `arrow` 53 with default features, which include `arrow::json` (`ReaderBuilder`, `Decoder::serialize`) (genson-core/Cargo.toml:8, 28)

## Missing

- No i9 timing of the whole claims step (Rust `[profile]` lines plus `read_parquet`, `json_decode`, `sink_parquet` in `normalise_claims_direct`) on any chunk, so the 4-core stage split is the only one available
- The `halfbrown` `fxhash` feature from 2026-09-24-schema-key-order-nondeterminism is not applied — genson-core/Cargo.toml has no `halfbrown` entry, so key order still varies between runs
- No Avro→Arrow `DataType` mapping in Rust
- No typed-Arrow output path from `normalise_from_parquet` — the output column holds JSON strings that the caller decodes with `str.json_decode`
- No bench covering `wrap_root` or the whole `normalise_from_parquet` path (read, infer, normalise, write) — `bench_claims` covers inference only (genson-core/examples/bench_claims.rs)
- No `SchemaNode::absorb` merging nodes directly, and no property test comparing a node-level merge against `add_schema(to_schema())`
- No byte-size-based batching for `max_builders` — chunks hold a fixed row count whatever the row sizes
