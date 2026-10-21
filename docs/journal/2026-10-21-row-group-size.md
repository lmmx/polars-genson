# 2026-10-21: Row-group size: investigated, not shipped

**Outcome:** genson keeps arrow-rs's 1Mi-row row groups. No code change was made, and the
rule that was built is kept as `logs/row_group_rule_not_shipped.diff`. The maintainer
decided to stop here. Scripts and logs are in `2026-10-21-row-group-size/`.

**Summary:**
1. **The first sweep (below) pointed to 128Ki rows,** close to Polars' own about 125k.
   At default settings it was faster than 1Mi on every synthetic table.
2. **The repeat runs didn't hold that up** (`confirm_rg.py`, `split_rg.sh`;
   `logs/confirm_rg_summary.txt`, `logs/split_rg_summary.txt`). At 128Ki rows:
   - `List(Struct)` and graded_tall got slower (1.12 to 1.53 s, and 0.82 to 0.95 s);
   - the graded tables' files grew 4 to 5%, because every row group rebuilds its
     dictionaries.
3. **Most of the flat-table gain was this VM's memory cost.** With transparent huge pages
   switched off for the benchmark process, a measurement setting only, row-group size
   made no difference to flat tables (`sweep_clean.sh`; `logs/sweep_clean_summary.txt`).
   So the default-settings gain came mostly from less kernel time on memory, not from
   extra parallelism.
4. **A shape-aware rule changed only two tables.** It cut row groups down (to no fewer
   than 128Ki rows) until a table had 4 column chunks per thread
   (`logs/row_group_rule_not_shipped.diff`). The two tables, at default settings
   (`confirm_shape.sh`; `logs/confirm_shape_summary.txt`):
   - `List(String)`: 1.32 to 0.60 s, against the Polars wheel's 0.63;
   - `List(Struct)`: 1.19 to 1.45 s, worse.

   Every other table, the Wikidata ones included, was byte-identical.
5. **The decision:** the only clear win was one shape, which doesn't justify a layout
   change.

The sections below are the investigation as it was written while in progress.

## Why: the original writer grid, rerun against the official Polars wheel

The #288 journal's grid (`2026-10-21-row-group-parallel-write/scripts/`) is rerun with
repeats (`grid_a.py`; `logs/original_grid_rerun.txt`). It compares genson's
`write_bench` example (original pre-#288 code and #292's head, input batches of a
sixteenth of the rows) against the official Polars 2.0.0 wheel's `sink_parquet` (zstd 1,
its default row groups): 5 rounds after a checked warm-up, and 10 for flat_1, flat_2 and
flat_94 (`grid_c.py` in the scratch run).

- **Polars today matches the original run:** flat_1 0.51 against 0.55 s, and claims 20.0
  against about 19 s. The machine is comparable.
- **The original grid was one pass per table:** the original genson code, rerun today, is
  1.6–1.9× behind on flat_4, flat_8 and flat_32, not the 1.2× it recorded.
- **At #292's head genson is still slower:** 1.2–1.55× on the flat tables and 1.6× on
  `List(String)`. It is level on `List(Struct)`, and 23× faster on claims.
- **The same-binary harness understated Polars:** the duel harness of
  `2026-10-21-adaptive-dictionary/threshold-sweep/` built Polars from crates, and that
  build is much slower than the official wheel. At the same row groups the wheel writes
  `List(String)` in 1.23 s, against the crate build's 2.93 s. #290's and #292's
  descriptions are corrected.

## Three causes of the remaining gap

**Kernel time spent on memory, varying run to run** (`rus.py`, `logs/faults_flat2.txt`):
- **Where the time goes:** genson's slow runs spend 2–5 s in the kernel, against 0.25 s
  in its fast runs and about 0.4 s in every Polars run. CPU time in genson's own code
  stays at 1.2–1.4 s throughout, below Polars' 1.4–1.8 s.
- **What causes it:** mimalloc, genson-core's global allocator, calls
  `madvise(MADV_HUGEPAGE)` on its 1 GiB arenas. A run takes about 473 transparent huge
  page faults, and Polars takes none.
- **Why it varies:** on this Firecracker VM a huge-page fault costs between 0.5 and 10 ms.
  There are no compaction stalls.
- **Huge pages off:** with them disabled for the process (`rus_nothp.py`, `thp_grid.sh`;
  `logs/huge_pages_off_grid.txt`), the ranges tighten and the flat gap is 1.25–1.4×.
- **Not pursued:** the allocator is left as it is, by the maintainer's decision.

**Too few tasks for the threads:**
- **How the writer splits work:** `write_batches` makes one task per (row group, leaf
  column), each encoded by one thread, in waves of one row group per thread.
- **What that leaves:** at arrow-rs's 1Mi-row row groups, `List(String)` (2.5M rows, one
  leaf) gives 3 tasks to 4 threads, 1.82 cores busy against Polars' 3.32.
- **What Polars does:** its default row groups hold about 125k rows. Forced to 1Mi rows,
  Polars slows 1.1–2× (`polars_write_rg.py` in the scratch run).

**Encoder CPU:** with huge pages off, genson's flat-table writes take 10–25% more CPU
than Polars' (1.47 against 1.32 s on flat_2). Not yet profiled.

## The sweep (`sweep_rg.py`; `logs/sweep_rg_summary.txt`)

**Setup:**
- **Build:** a scratch build reads the row-group size from `GENSON_RG_ROWS`
  (`logs/rg_rows_from_env.diff`). Waves hold one thread's worth of 1Mi rows, so memory in
  flight is as before.
- **Sizes:** 1Mi, 512Ki, 256Ki, 128Ki, 64Ki and 32Ki rows, against the Polars wheel at
  its defaults.
- **Runs:** 5 rounds of fresh processes in rotated order.
- **Checks on each warm-up output:** it equals its input (`check.py`), and it is read
  3 times by Polars and by pyarrow.

Median write wall time in seconds:

| table | Polars wheel | 1Mi (#292) | 512Ki | 256Ki | 128Ki | 64Ki | 32Ki |
|---|---|---|---|---|---|---|---|
| flat_1 | 0.498 | 0.694 | 0.719 | 0.545 | **0.484** | 0.520 | 0.511 |
| flat_2 | 0.368 | 0.535 | 0.482 | 0.368 | 0.350 | 0.318 | 0.337 |
| flat_4 | 0.358 | 0.564 | 0.460 | 0.368 | **0.324** | 0.334 | 0.395 |
| flat_8 | 0.309 | 0.554 | 0.445 | 0.306 | 0.398 | 0.330 | 0.337 |
| flat_32 | 0.327 | 0.514 | 0.505 | 0.298 | 0.382 | 0.326 | 0.349 |
| flat_94 | 0.359 | 0.602 | 0.425 | 0.363 | **0.297** | 0.386 | 0.522 |
| `List(String)` | 0.652 | 1.520 | 0.664 | 0.908 | 0.853 | 0.814 | 0.648 |
| `List(Struct)` | 1.327 | 1.049 | 0.899 | 0.821 | 0.834 | 0.822 | 0.830 |

**File size (MB):**

| table | 1Mi | 128Ki | 64Ki | 32Ki |
|---|---|---|---|---|
| flat_1 | 90.05 | 90.26 | 90.30 | 91.07 |
| flat_2 | 62.38 | 62.49 | 63.22 | 64.95 |
| flat_8 | 55.05 | 55.34 | 55.99 | 57.91 |
| flat_94 | 58.34 | 58.60 | 59.22 | 60.54 |

**Results:**
- **Every size from 256Ki down is faster than 1Mi on every table.** The wide tables gain
  as much as the narrow ones: flat_94 goes from 0.60 to 0.30 s at 128Ki. So the size
  doesn't need to depend on the column count.
- **Below 128Ki files grow:** 128Ki adds at most 0.4%, 64Ki up to 1.4%, and 32Ki up to 4%.
- **Reads are as fast or faster at 128Ki, in both Polars and pyarrow.** flat_1 reads in
  0.19 s against 0.32 s at 1Mi (Polars), and 0.22 against 0.53 (pyarrow).
- **Chosen size:** 128Ki rows, close to Polars' own about 125k.
