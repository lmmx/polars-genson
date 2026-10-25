# 2026-10-21: Why Polars writes Wikidata claims 23× slower than genson

Chunk 0's claims table has 10,000 rows. It is two top-level columns, `id` and `claims`.
`claims` holds 94 leaf columns, with lists nested up to 5 deep. Both writers are timed
writing it alone, at zstd 1, against the official Polars 2.0.0 wheel (Rust source 0.55.1).
Scripts and logs are in `2026-10-21-polars-claims/`.

| writer | wall | write CPU | cores busy |
|---|---|---|---|
| Polars wheel | 19.7 s | 19.5 s | 0.99 |
| genson (#292) | 0.90 s | 3.3 s | 3.6 |

The 23× splits into about 3.6× of parallelism and about 6× of CPU.

## Parallelism: Polars' unit of work is the top-level column

- **The design:** polars-stream's `row_group_encoder.rs` runs one task per top-level
  column. Inside that task it encodes the column's leaves one after another. Claims is
  essentially one top-level column in one row group, so one thread does all of it.
- **genson's unit:** genson's `write_batches` makes one task per (row group, leaf
  column), which gives claims 94 tasks.
- **Tested** (`dup_distinct.py`; `logs/dup_distinct.txt`): copies of `claims` were added
  as more top-level columns, each read from the file separately so that no two share
  buffers.

| top-level copies | wall | CPU | cores busy |
|---|---|---|---|
| 1 | 17.0–18.2 s | 16.3–17.5 s | 0.99 |
| 2 | 17.9–19.2 s | 34–36 s | 1.95 |
| 4 | 19.6–20.0 s | 68–69 s | 3.79 |

- **Result:** each top-level column gets its own core, and wall time stays level. A table
  with several nested top-level columns would get some parallelism from Polars. Claims
  doesn't, by its shape.
- **Shared-buffer artefact:** copies made with `pl.col("claims").alias(...)` share their
  buffers (`dup.py`; `logs/dup.txt`). Then each concurrent encode runs about 2.7× slower:
  two copies took 46 s and 90 s of CPU, and four took 67–73 s and 206–242 s. The cause is
  most likely contention on shared cache lines, but that's not verified. It matters only
  to tables whose columns alias one another.

## CPU: the Dremel level iterator, three walks per page

- **Profile:** 35 gdb samples of every thread during the write (`sample.sh`), classified
  by caller (`classify.py`; `logs/classify.txt`). One thread was busy in 34 of the 35.
  The wheel ships a symbol table, but the iterator's own frames are inlined, so samples
  are attributed by their caller.

| where the busy thread was | share |
|---|---|
| `BufferedDremelIter::next`, under `write_rep_and_def`, def levels | 40% |
| `BufferedDremelIter::next`, under `write_rep_and_def`, rep levels | 26% |
| `BufferedDremelIter::next`, called directly from the leaf's `array_to_page` (the `num_values` count) | 29% |
| zstd | 3% |
| string views to Arrow strings | 3% |

- **In the iterator:** 94% of Polars' time is in `BufferedDremelIter`.
- **Three walks per page:** for each page of each nested leaf, polars-parquet builds three
  iterators over the leaf's full path from the root: `num_values`
  (`BufferedDremelIter::new(nested).count()`), the rep levels and the def levels
  (`nested/mod.rs`, `nested/dremel/mod.rs`). The samples split about one third and two
  thirds between the count and the rep and def walks, as that predicts.
- **What the walk costs:** each walk steps one value at a time through every list and
  struct level above the leaf, its offsets and validity. Every leaf repeats the walk over
  their shared ancestors. Claims has 94 leaves under up to 5 list levels, so the ancestors
  are walked about 94 × 3 times per page.
- **What genson does instead:** arrow-rs (`compute_leaves`) computes each leaf's levels
  once, into level buffers that the encoder slices. An earlier single-threaded callgrind
  of genson's claims write put `compute_leaves` at 21% of instructions.
- **Implication:** about a third of Polars' CPU on claims is a count it could get from the
  level buffers it builds next, and the rest is the cost per element of a walk that goes
  one value at a time. Fixing it means redesigning how levels are computed, not speeding
  up one branch.

## Scaling: linear in rows

`scale.py` (`logs/scale.txt`) writes the first n rows, two runs each:

| rows | CPU |
|---|---|
| 1,250 | 3.5 s |
| 2,500 | 6.1–6.2 s |
| 5,000 | 9.2 s |
| 10,000 | 17.4–17.5 s |

Each doubling costs at most 1.9×, and the first rows are the heaviest entities. That's
consistent with `row_slice_ranges` cutting pages by top-level rows and each walk covering
its own page. There is no sign of superlinear cost.

## Limits

- **Sample size:** 35 samples put about ±8 percentage points on each share.
- **Effect on wall time:** gdb's pauses stretched that write to 38.7 s of wall time, but
  its CPU (17.0 s) matches unsampled runs.
- **genson's side:** the comparison with genson's level computation rests on arrow-rs's
  design and that earlier callgrind, not on a new profile of genson.
