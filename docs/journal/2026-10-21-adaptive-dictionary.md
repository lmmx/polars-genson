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

## The threshold

A sweep of the distinct ratio above which a leaf is written plain: 0, 0.25, 0.5, 0.75,
0.9 and 1 (`threshold-sweep/`).

- **One build for both writers:** `duel.rs` links this branch's genson-core, with the
  threshold read from `GENSON_DICT_THRESHOLD` (`logs/threshold_from_env.diff`). It also
  links Polars 0.55.2, the crate behind Polars 2.0.0's `sink_parquet`.
  - It is built with the workspace's release profile and genson's allocator (mimalloc),
    and starts both thread pools before anything is timed.
  - It times the write alone, of a table already in memory: zstd 1 and row groups of 1Mi
    rows for both writers.
- **Rounds** (`sweep.py`): on a table, thresholds that make the same decision for every
  leaf write the same file (checked with `cmp`), so each such group is timed once. Each
  group and Polars get 5 rounds of fresh processes, in rotated order, after a warm-up run.
- **Correctness** (`check.py`): every output, at thresholds 0, 0.75 and 1 and from Polars,
  equals its input.
- **Tables:**
  - the synthetic tables of the parent's measurements (`gen.py`): flat, 2 columns of 12.5M
    rows; flat, 94 columns; `List(String)`; `List(Struct)`. Every leaf is more than 90%
    distinct in its sample, so thresholds 0 to 0.9 decide alike there.
  - `graded_tall` (2,097,152 rows, 18 columns) and `graded_wide` (419,430 rows, 90
    columns) (`gen_graded.py`). Each column is drawn from K distinct values, as Int64,
    short and long strings, and samples at 6, 49, 69, 85, 96 or 100% distinct.
  - chunk 0's claims, labels, descriptions and aliases. Claims' 73 sampled leaves spread
    across the range (`logs/ratios/`).

File size in MB at each threshold, with its excess over the table's smallest (`+` marks
user CPU slower than at 0.75, with disjoint ranges; `logs/summary.txt`):

| table | 0 | 0.25 | 0.5 | 0.75 | 0.9 | 1 | Polars |
|---|---|---|---|---|---|---|---|
| labels | 4.35 (+6.2%) | 4.09 | 4.09 | 4.14 (+1.2%) | 4.14 (+1.2%) | 4.16 (+1.6%) | 4.36 |
| aliases | 1.76 (+2.3%) | 1.72 | 1.72 | 1.80 (+4.7%) | 1.80 (+4.7%) | 1.81 (+5.7%) | 1.75 |
| descriptions | 4.59 (+2.6%) | 4.48 | 4.48 | 4.48 | 4.48 | 4.50 (+0.5%) | 4.60 |
| `List(String)` | 80.40 | 80.40 | 80.40 | 80.40 | 80.40 | 80.90 (+0.6%) | 80.32 |
| `List(Struct)` | 113.34 | 113.34 | 113.34 | 113.34 | 113.34 | 114.68 (+1.2%) | 113.36 |
| flat, 2 columns | 67.60 | 67.60 | 67.60 | 67.60 | 67.60 | 72.14+ (+6.7%) | 62.62 |
| flat, 94 columns | 60.16 | 60.16 | 60.16 | 60.16 | 60.16 | 79.89+ (+32.8%) | 58.67 |
| graded_tall | 92.45 (+20.1%) | 87.42 (+13.6%) | 83.02 (+7.9%) | 78.71 (+2.3%) | 76.95 | 78.56 (+2.1%) | 70.12 |
| graded_wide | 96.81+ (+16.8%) | 91.81 (+10.8%) | 87.64 (+5.8%) | 83.80 (+1.1%) | 82.86 | 87.32+ (+5.4%) | 81.73 |
| claims | 57.63 (+7.2%) | 53.89 (+0.2%) | 53.78 | 54.08 (+0.6%) | 54.24 (+0.9%) | 54.34 (+1.0%) | 59.02 |

- **Time:** from 0.25 to 0.9, user CPU time on every table falls within the range of the
  time at 0.75. Only 0 (on graded_wide) and 1 (on both flat tables and graded_wide) are
  slower.
- **Size:** no threshold gives the smallest file everywhere.
  - 0.5 gives the smallest of all four Wikidata tables: 64.07 MB together, against 64.50
    at 0.75 (0.7%). It is 7.9% larger on graded_tall.
  - 0.9 gives the smallest graded tables, 1.1 to 2.2% under 0.75, and a larger claims file.
  - 0.75 and 0.9 share the smallest worst case, 4.7% over the best (aliases, 80 kB).
- **The threshold stays at 0.75:** no other is faster anywhere, and each is smaller only
  on some tables and larger on others.
- **Why no ratio fits every table:** a sample's distinct ratio sees neither of the two
  things that decide between dictionary and plain.
  - It cannot see how close together the repeats are. Repeats in nearby rows (an entity's
    label in many languages) are found by zstd in plain pages. Repeats spread over a
    million rows (the graded columns) are found only by a dictionary.
  - It cannot see the chunk's cardinality. A sample of 16,384 values sees a column of
    50,000 values over 1Mi rows as 85% distinct.
- **What Polars does instead:** it estimates the cardinality of the whole chunk
  (HyperLogLog), so it dictionary-encodes graded_tall's 50k- and 200k-value columns.
  These account for 7.3 of the 8.6 MB by which its file is smaller (`logs/columns.txt`).
  - At any threshold, arrow-rs's 1 MiB dictionary page limit would turn the longer of
    those dictionaries back to plain.
  - A cost estimate made from the same sample inherits both blind spots. The two could be
    closed by an estimate over the whole chunk with a larger dictionary page limit. On the
    Wikidata tables genson's files are already smaller than Polars' everywhere but aliases
    (1.80 against 1.75 MB), so that is left undone.
- **Sampling** (median wall time of the parallel sampling):
  - 1.3 to 1.8 ms on the 10,000-row maps: 2 to 5% of their 30 to 65 ms writes, which vary
    more than that from run to run. There it buys under 1% of size (0.75 against 1) and no
    measurable time.
  - 0.7 to 1.5 ms on the synthetic tables of 1 or 2 columns; 10 to 13 ms on 90 or 94
    columns; 15 to 16 ms on claims (2% of 0.85 s). On 94 columns it buys 0.65 s of CPU
    time and 25% of size.

At 0.75 against Polars (`logs/vs_polars.txt`):

| table | user CPU | wall time |
|---|---|---|
| claims | 2.91 s against 23.06 s | 0.85 s against 26.48 s |
| labels, descriptions, aliases | faster | faster, or within noise |
| `List(String)`, `List(Struct)` | faster | 0.89 and 1.07 s against 2.84 and 4.69 s |
| graded_tall, graded_wide | faster | within noise |
| flat, 2 and 94 columns | within noise | within noise |

- Genson is faster on nested and Wikidata tables, and level on flat ones.
- On the flat tables both writers write every column plain. Their files differ in the
  plain columns: the Int64 column of the 2-column table is 21.4 MB from genson and 17.5 MB
  from Polars, with the same encoding (`logs/columns.txt`). This is not the dictionary
  decision. arrow-rs's 20,000-row data page limit, against Polars' 1 MiB pages, is the
  likely cause and is not yet tested.

## Current State

- `test_mostly_distinct_leaves_are_not_dictionary_encoded` (genson-core/src/tests/
  parquet.rs) checks the dictionary pages of a unique and a repeated top-level string, a
  unique integer and a repeated string in a list of structs, and a JSON string column;
  applying no decision fails it.
- `write_batches` is unchanged: `test_write_batches_matches_arrow_writer` still compares
  it with `ArrowWriter` under the same properties.

## Missing

- A column that repeats only after its first 16,384 values is judged on that sample, as
  is one whose values repeat across the chunk more than within the sample (see The
  threshold).
- Plain columns are larger than Polars' on flat tables (8% on the 2-column table), for a
  reason other than the dictionary decision.
- Leaf types other than strings, binaries, 32/64-bit integers and floats keep arrow-rs's
  default (dictionary on, except booleans).
