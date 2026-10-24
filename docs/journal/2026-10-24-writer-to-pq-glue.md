# 2026-10-24: genson's Parquet writer moved to pq-glue

Continues docs/journal/2026-10-23-arrow-output.md.

## Current State

- genson-core's `parquet` feature depends on pq-glue 0.2.0 from crates.io (`write`
  feature; lmmx/pq-glue#2, its `src/write.rs` the same as at e47f28b, the commit this
  branch was first pinned to). Every Parquet file genson
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

Checked again on pq-glue 0.2.0: a harness writing the same data through this branch and
through master (genson's own writer) gave 16 files byte for byte the same (struct columns
in all four layouts, with and without native maps, `write_arrow_batches` with one, many
and tiny row groups, and string columns, empty included), on 1, 3 and 4 threads.

## Missing

- Nothing for the move. Not carried over: #294 (levels computed a few batches at a time)
  was never merged into genson's writer, so pq-glue's writer does not have it either.
