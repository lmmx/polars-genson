# 2026-10-22: Levels computed and written a few batches at a time

genson-core's Parquet writer (genson-core/src/parquet.rs, `write_batches`) computes the
definition and repetition levels of up to one batch per thread at a time, and hands them to
the column writers before computing the next batches'. Before, it computed every batch's
levels in a row group first, then wrote them all.

The files are unchanged byte for byte. On the largest Wikidata chunks, wikidata-pq's
`process()` takes up to 28% less time, its system CPU drops by most of its share, and peak
memory falls 10–18%. Scripts and logs are in `2026-10-22-level-waves/`.

## Why: profiling wikidata-pq's `process()` on 1.2.2

**Setup:**
- **Code:** wikidata-pq master f976afe with its `uv.lock` moved to polars-genson 1.2.2
  (lmmx/wikidata-pq#19), in an environment installed from that lock with
  `--require-hashes`. Polars 2.0.0, pyarrow 25.0.1, 4 cores.
- **Chunks:** 11 routed chunks of release 20261012:
  - c0, its chunk 0;
  - r10–r90 (general) and s10–s90 (scholarly articles), each routed from a 10,000-entity
    window at that fraction of the dump.
- **Runs:** one fresh process per chunk, as wikidata-pq's `pool.py` runs them, warm page
  cache, 3 rounds in rotated order (`prof.py`).
- **Measurements:**
  - `prof.py` wraps each step of `process()`: wall, CPU (user and system), minor faults,
    peak RSS.
  - It passes `profile=True` to every `normalise_from_parquet` call, for genson's stage
    timings.

**Results** (`logs/baseline_profile_1.2.2.md`):
- **Total:** `process()` takes 12.3 s summed over the chunks (median per chunk).
- **The claims step dominates:** genson's claims `normalise_from_parquet` is 76% of it.
- **Inside claims, the write is largest:** the write stage is 49% of the step on c0 and
  37% on the others; decode, inference and invariant extraction are 13–23% each.
- **Imports,** outside `process()`: 0.34–0.45 s in each worker. On the ten typical chunks
  that is 3.8 s, against 5.7 s of `process()`.

### Where the write's time goes

**System CPU** (`timeline.py`; `logs/c0_s90_stage_user_sys.txt`):
- **Sampling:** another process samples `/proc/<pid>/stat` every 20 ms, and genson's stage
  durations are laid end to end from the claims call's start.
- **c0's write:** 2.7–2.8 s of user CPU, 1.8–4.1 s of system CPU, and 29–40k minor faults.
  System CPU is about 40% of its CPU even in a good run.
- **Variance:** in a slow run every stage's system CPU grows. That is the run-to-run spread.
- **s90's write:** nearly all user CPU.

**gdb samples of every thread** (`gdbsampler.py`, `sample2.sh`, `classify.py`;
`logs/write_samples_classified.txt`):
- **The build:** genson 1.2.2 (e5d2e6e), built as the release builds it (`opt-level = 2`,
  thin LTO, one codegen unit, default features) but with symbols and line tables kept.
  Its claims step times the same as the PyPI wheel's (`logs/symbolised_build_parity.txt`).
- **The sampler:** one gdb attaches with only genson's symbols, and the shell sends it
  SIGINT every 50 ms.
- **Stack depth matters:** stacks must be deep. At `bt 40`, the deeply recursive level code
  never reaches the `write_batches` frame, and its threads look idle. That once suggested
  the write kept only two threads busy. At `bt 150` both of its phases keep 3.7–3.9 of 4
  threads busy (`logs/write_phase_parallelism.txt`).

At `bt 150`, the share of busy thread samples in the write:

| | c0 | s90 |
|---|---|---|
| allocator: fresh pages (`mi_page_free_list_extend`), `realloc` copies, new segments | 50% | 61% |
| level computation (`compute_leaves`) | 19% | 15% |
| level encoding (RLE / bit-packing) | 15% | 10% |
| zstd | 7% | 5% |
| everything else | 9% | 9% |

- **Who allocates:** the allocator time is asked for mostly by arrow-rs's level builder
  (`write_leaf`, `append_run`, `append_rep_level_run`, `resize`, `materialize_mut`), and
  less by value encoding and page compression.
- **How the builder stores levels:** parquet 60 keeps each leaf column's levels in its own
  `Vec<i16>` per level kind, and its non-null indices in a `Vec<usize>`, grown by `extend`.
- **How much there is** (`level_counts.py`; `logs/c0_claims_level_counts.txt`): c0's claims
  are one row group of 10,000 rows. Its 94 leaf columns hold 71M level entries, 79% of them
  null.

**The cause:** `write_batches` computed the levels of every batch of a row group before any
column writer ran. All 16 batches' levels (a decode makes 4 per thread) were alive at once,
each in fresh allocations that the kernel had to fault in.

## Change

- **Steps:** within each wave of row groups, `write_batches` now goes through the batches in
  steps.
- **Step size:** each step takes the next `max(threads / row groups in the wave, 1)` batches
  of every row group in the wave, about one batch per thread in all.
- **Each step:** it computes those batches' levels in parallel (each batch a task, as
  before), then passes them to each column writer in parallel (each (row group, leaf) a
  task, as before).
- **After the last step:** the writers are closed in parallel, and their chunks appended in
  row group order, as before.
- **Why the bytes cannot change:** each column writer still gets its leaf of every batch in
  order, so its pages and the file are the same.
- **Effect:** at most about one batch per thread has its levels held at once, and a step
  reuses the memory the last one freed.

## Correctness

- **Byte identity with arrow-rs:** `test_write_batches_matches_arrow_writer` still passes.
  It compares `write_batches` byte for byte with `ArrowWriter` across four writer layouts,
  seven row counts (row groups of many uneven batches) and pools of 1, 3 and 8 threads.
- **Real data:** across the 11 chunks, every output of `process()` is byte-identical
  between 1.2.2 and this change: 77 files, all seven tables (`logs/ab_process_11_chunks.md`).
- **Tests:** genson-core's suite passes with `parquet,avro`: 142 library tests (3 ignored)
  and the integration tests.
- **Lint:** `cargo clippy -p genson-core --all-targets --features parquet,avro -- -D
  warnings` and `cargo fmt --check` are clean.

## Measured: wikidata-pq `process()`, A against B

**Setup** (`ab_run.sh`, `ab_compare.py`, `ab_summary.py`):
- **A and B:** A is 1.2.2 as built above. B is the same build of this change.
- **Runs:** one fresh process per chunk, alternating A and B by chunk and round.
- **Rounds:** 7 for the four largest chunks (c0, s30, s70, s90), 7 for the six smallest,
  and 3 for s10.

Medians, with ranges (`logs/ab_summary.md`):

| chunk | `process()` wall (s) | claims system CPU (s) | write stage (s) | peak RSS (MB) |
|---|---|---|---|---|
| c0 | 5.49 (5.15–7.03) → **3.97** (3.61–7.01) | 6.22 → 1.36 | 1.83 → 1.04 | 2,069 → 1,706 |
| s90 | 2.02 (1.50–3.03) → **1.54** (1.29–3.10) | 2.70 → 0.37 | 1.05 → 0.48 | 1,387 → 1,217 |
| s30 | 1.38 (1.11–2.39) → 1.40 (1.02–1.81) | 1.01 → 0.71 | 0.34 → 0.34 | 1,177 → 1,024 |
| s70 | 0.81 (0.69–1.59) → 0.80 (0.67–1.82) | 0.46 → 0.35 | 0.27 → 0.22 | 795 → 712 |

- **The largest chunks:** `process()` takes 28% (c0) and 24% (s90) less time. Their write
  stage falls 43% and 54%, and the claims step's system CPU 78% and 86%.
- **The medium chunks** (s30, s70): wall time is unchanged, and system CPU and peak memory
  fall.
- **Peak RSS** falls 10–18% on the four.
- **User CPU is unchanged,** within ±8% with no consistent direction on every chunk: the
  change saves fault work, not computation.
- **The six smallest chunks** (0.1–0.6 s): system CPU there varies more between runs than
  any difference. The best runs of each are level: within 0.02 s on four, and better on
  r10 and r30.
- **Outliers:** each side has runs where system CPU spikes for the whole process (c0 B
  7.01 s, A 7.03 s). That is the VM's fault cost, which varies run to run (2026-10-21
  journal, row-group size).

## Limits

- **One machine:** a 4-core Firecracker VM with transparent huge pages, where faults are
  expensive and vary. On a machine where faults are cheap, the gain is smaller.
- **Allocator:** untouched. mimalloc and its settings are as they were.
- **What remains of the write:** level computation, level encoding and zstd are arrow-rs's.
  The level builder's growth by `extend` is upstream code.
