# 2026-10-24: genson's Parquet writer moved to pq-glue

Continues docs/journal/2026-10-23-arrow-output.md.

## Current State

- genson-core's `parquet` feature depends on pq-glue (`write` feature), from its branch
  (lmmx/pq-glue#2, merged; pinned to its master commit e47f28b) until it is published to crates.io. Every Parquet file genson
  writes (`write_string_column_with`, `write_struct_columns`, `write_arrow_batches`, so
  `write_parquet` and the normalised, lookup and pruned outputs) goes through
  `parquet::write`, which calls `pq_glue::write::write_batches` with genson's settings:
  `spec_names` as before (off for string columns, the layout's for struct columns, on
  for `write_arrow_batches`), the row group size, and pq-glue's defaults, which are
  genson's writer's (zstd 1, adaptive dictionary, pages ended at the size limit alone).
- Removed from genson-core: `compression`, `open_writer`, `DICTIONARY_SAMPLE`,
  `unrepeated_leaves`, `leaf_arrays`, `sample` and `write_batches`, which pq-glue holds
  now, with the test of `write_batches` against `ArrowWriter` (moved to pq-glue's
  src/write.rs). genson-core's Parquet tests pass (12, one ignored as before).
- Genson calls the Rust API with its record batches: no Python and no FFI between them.

## Why

One native writer, owned by pq-glue, serving genson (from Rust) and wikidata-pq (from
Python, `pq_glue.write_parquet`), in place of a writer in a JSON-schema crate that a
sort of Parquet files had to reach through genson for. pq-glue's journal
(docs/journal/2026-10-24-writer-engines.md) has the design, the engines and the
measurements.

## Measured

wikidata-pq's benchmark claims key (1,175,112 rows, 93 leaf columns, row groups of
158,708), `write_parquet` of the table, 3 runs each in a fresh process, median, on this
session's 4-core VM:

| | wall | CPU | RSS above the table |
|---|---|---|---|
| polars-genson 1.3.3 | 2.91 s | 9.1 s | 870 MB |
| this branch (genson on pq-glue) | 2.47 s | 7.6 s | 887 MB |
| pq-glue's own wheel, the same call | 1.68 s | 5.7 s | 554 MB |

All three files are the same, byte for byte (sha256 `dc1a46443650e613…`). The move
costs genson nothing. The same code takes longer and holds more in genson's process
than in pq-glue's: genson-core installs mimalloc as the global allocator, pq-glue the
system's (not changed here).

## Missing

- pq-glue published to crates.io with `write`, and genson-core's dependency moved from
  the branch to that version, before this merges.
