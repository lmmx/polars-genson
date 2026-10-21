# 2026-10-21: Parquet row groups encoded side by side

genson-core's `write_batches` (genson-core/src/parquet.rs) encodes up to one row group per
thread at a time, each (row group, leaf column) pair and each batch's level computation a
task of its own, and writes the encoded column chunks in row group order. Before, it
encoded one row group at a time with its leaf columns in parallel, so a narrow table ran
on one thread. The file is unchanged, byte for byte. Scripts are in
`2026-10-21-row-group-parallel-write/scripts/`, outputs in `logs/`.

## Why

Writing the tables of `scripts/gen.py` (about 200 MB of values each: flat tables of 1 to
94 columns of `Q<n>` strings and Int64s, a `List(String)` and a
`List(Struct{a: String, b: Int64})` column) with genson's writer alone
(`scripts/write_bench.rs`, batches of a sixteenth of the rows, as `normalise_from_parquet`
makes them) and with Polars 2.0.0's `sink_parquet` (zstd 1), one run each on 4 threads
(`logs/polars_vs_genson_before.txt`):

| table | Polars wall / CPU | genson wall / CPU |
|---|---|---|
| flat, 1 column (25M rows, 24 row groups in genson) | 0.55 / 1.6 s | 1.87 / 1.9 s |
| flat, 2 columns | 0.31 / 1.1 s | 1.07 / 1.5 s |
| flat, 94 columns (1 row group) | 0.37 / 1.2 s | 0.87 / 3.3 s |
| `List(String)` | 0.73 / 2.4 s | 1.42 / 1.4 s |
| `List(Struct)` | 1.37 / 4.5 s | 1.67 / 2.5 s |

- On a narrow table genson's wall time equals its CPU time: its row groups were encoded
  one after another and each one's single column on one thread. Polars encodes its row
  groups (about 123k rows each, 203 for 25M rows) several at a time.
- On chunk 0's Wikidata claims (10,000 rows, one row group, 94 leaf columns, 5 list
  levels) Polars' sink takes about 20 s on one core, genson 0.75 to 1.1 s: Polars encodes
  each top-level column's leaves serially (polars-stream `row_group_encoder.rs`) and
  walks each leaf's nesting three times per page (rep levels, def levels, a value count;
  88% of its write's instructions under callgrind, in `BufferedDremelIter::next`).

## Measured against the parent

`scripts/ab.sh`, one run of each writer per table, 4 threads; `logs/ab_results.txt`:

| table | row groups | before wall / CPU | after wall / CPU | file |
|---|---|---|---|---|
| flat, 1 column | 24 | 2.41 / 2.6 s | 0.81 / 2.5 s | identical |
| flat, 2 columns | 12 | 0.93 / 1.3 s | 0.42 / 1.4 s | identical |
| flat, 4 columns | 6 | 0.49 / 1.3 s | 0.47 / 1.5 s | identical |
| flat, 8 columns | 3 | 0.45 / 1.5 s | 0.40 / 1.3 s | identical |
| flat, 32 columns | 1 | 0.43 / 1.5 s | 0.59 / 2.0 s | identical |
| flat, 94 columns | 1 | 1.22 / 4.3 s | 0.61 / 2.2 s | identical |
| `List(String)` | 3 | 2.31 / 2.6 s | 0.99 / 2.0 s | identical |
| `List(Struct)` | 3 | 3.00 / 5.4 s | 2.51 / 6.1 s | identical |
| claims (chunk 0) | 1 | 0.79 / 2.9 s | 0.89 / 3.2 s | identical |

- Tables of several row groups and few columns take a third to a half of the time. A
  table of one row group goes through the same work as before; its differences are run
  to run variation, which reached ±50% on this machine (claims, three alternating runs:
  before 1.88, 0.82, 0.80 s; after 0.90, 1.39, 0.74 s).
- Each output equals its input (pyarrow `Table.equals`), and equals the parent's output
  byte for byte (`cmp`).
- `test_write_batches_matches_arrow_writer` (genson-core/src/tests/parquet.rs) compares
  the file with `ArrowWriter`'s byte for byte in thread pools of 1, 3 and 8 threads;
  appending the chunks in reverse fails it.

## The 94-column table

- One-threaded, 94 flat columns of 265,957 rows took 3.2 to 4.5 s of CPU and 32 columns
  of 781,250 rows (the same 25M values) 1.3 to 2.8 s, varying from run to run.
- Under callgrind the two writes are 16.5G and 14.4G instructions (14% apart, reading
  included), with 69,409 and 42,804 minor page faults and 1,134 and 810 MB peak memory:
  the difference is in memory (94 column writers' buffers) rather than work done.

## Current State

- `write_batches` holds at most `rayon::current_num_threads()` row groups' levels and
  encoded chunks before writing them (parquet.rs, `in_flight`), where it held one.

## Missing

- Row groups are encoded in waves: a wave's slowest row group holds up the next.
- The 94-column memory overhead is not reduced.
