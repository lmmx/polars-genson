# 2026-10-21: Dictionary encoding only where values repeat

genson-core's Parquet writer (genson-core/src/parquet.rs, `open_writer`,
`unrepeated_leaves`) samples the first 16,384 non-null values of each leaf column before
it opens the file, and turns dictionary encoding off for a leaf whose sample is more than
three quarters distinct (of at least 128 values sampled), at any depth. Polars skips
dictionaries at the same ratio (polars-parquet `encode_as_dictionary_optional`, a
cardinality estimate over 0.75 of the length). Before, every leaf was dictionary encoded
(arrow-rs's default) until its dictionary reached 1 MiB, then fell back to plain.
Measurements are in `2026-10-21-adaptive-dictionary/`.

## Why

genson's writer alone against Polars 2.0.0's `sink_parquet` (zstd 1), after #288, on
the tables of `2026-10-21-row-group-parallel-write/scripts/gen.py` and chunk 0's claims,
with arrow-rs settings set from the environment in a scratch build
(`writer_options.txt`): with dictionaries off, genson's writes of flat tables matched or
beat Polars' (1 column 0.55-0.58 s against 0.51; 94 columns 0.36 s against 0.43), where
with them on they took 0.90-0.92 s and 0.72-1.54 s. The near-unique synthetic columns
also came out larger with dictionaries (94 columns: 79.9 MB against 60.2 MB off), and
real claims smaller (54.3 MB against 57.6 MB off): some of their leaves repeat (property
ids, snak types, ranks) and some do not (statement ids, hashes).

## Measured against the parent

One write of each table by each writer in ABBA order (`ab.txt`), wall s and file MB:

| table | before | after | before MB | after MB |
|---|---|---|---|---|
| flat, 1 column (25M rows) | 0.76, 0.75 | 0.50, 0.81 | 95.93 | 92.40 |
| flat, 2 columns | 0.70, 0.40 | 0.43, 0.39 | 72.14 | 67.60 |
| flat, 94 columns | 1.36, 1.07 | 0.44, 0.46 | 79.89 | 60.16 |
| `List(String)` | 0.77, 1.29 | 1.16, 1.24 | 80.90 | 80.40 |
| `List(Struct)` | 1.58, 1.25 | 1.62, 1.11 | 114.68 | 113.34 |
| claims (chunk 0) | 0.81, 0.83 | 0.83, 1.01 | 54.34 | 54.08 |
| labels | 0.08, 0.06 | 0.07, 0.07 | 4.16 | 4.14 |
| descriptions | 0.05, 0.06 | 0.07, 0.06 | 4.50 | 4.48 |
| aliases | 0.03, 0.03 | 0.03, 0.03 | 1.81 | 1.80 |

- No file is larger; the 94-column table is 25% smaller and 2.5 times faster to write.
- CPU, two runs each in alternation: claims 3.18 s before, 2.76 and 3.34 s after (a
  first-run 7.66 s aside); flat, 2 columns 1.55 and 1.71 s before, 1.16 and 1.20 after.
- Every output equals its input (pyarrow `Table.equals`).
- The JSON-string output of chunk 0's claims is unchanged in size (45.8 MB): JSON rows fill
  a 1 MiB dictionary within a few rows, after which arrow-rs falls back either way.

## Current State

- `test_mostly_distinct_leaves_are_not_dictionary_encoded` (genson-core/src/tests/
  parquet.rs) checks the dictionary pages of a unique and a repeated top-level string, a
  unique integer and a repeated string in a list of structs, and a JSON string column;
  applying no decision fails it.
- `write_batches` is unchanged: `test_write_batches_matches_arrow_writer` still compares
  it with `ArrowWriter` under the same properties.

## Missing

- A column that repeats only after its first 16,384 values is judged on that sample.
- Leaf types other than strings, binaries, 32/64-bit integers and floats keep arrow-rs's
  default (dictionary on, except booleans).
