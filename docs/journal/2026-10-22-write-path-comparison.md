# 2026-10-22: What is left to take out of process(): arrow-rs's writer, genson's stages

**What this entry is:** a comparison of where wikidata-pq's `process()` could still get
faster after the level waves (2026-10-22-level-waves):
- inside arrow-rs's Parquet writer;
- in genson's own decoding, inference and extraction.

**What it does not do:** change genson. Every experiment was a disposable build. Scripts and
logs are in `2026-10-22-write-path-comparison/`.

**Findings:**
- **Shared repetition levels inside arrow-rs:** leaves under the same list have the same
  repetition levels, and computing them once is the one large algorithmic saving found.
  - Write stage: −21% on chunk 0, −36% on s30.
  - `process()`: −6% to −9% on the large chunks.
  - Output: byte-identical files.
  - Route: it needs a change to arrow-rs's internals. Through arrow-rs's public API it would
    mean genson computing all levels itself.
- **Genson's stages:** the copies they make are mostly paid for by memory a later stage
  reuses. Removing extraction's copy saved CPU but no time (discarded).
- **Measurement:** on this VM, a process's time depends heavily on how much memory the
  process before it used. Comparisons must control for it (see the next section).

## Setup

- **Baseline:** polars-genson #294 (perf/level-waves, d5b9da6), built with symbols as the
  release profile builds it, wikidata-pq at 2599c1e. Polars 2.0.0, pyarrow 25.0.1, 4 cores.
- **Chunks:** the 11 of 2026-10-22-level-waves: c0, and r10–r90 and s10–s90.
- **Runs:**
  - `process()` in a fresh process per run (`prof.py` of 2026-10-22-level-waves).
  - Variants run in an order drawn afresh per chunk and round (`mv_run_primed.sh`).
  - Each run is preceded by `primer.py`, except where a log says unprimed.
  - Results are paired by round (`mv_compare.py`): each variant against the reference run on
    the same chunk in the same round, median and 90% bootstrap interval.
- **Allocator:** mimalloc and its settings untouched throughout.

## A measurement trap: the memory the last process used

The same build on the same chunk (`logs/memory_history.txt`):

| c0 run follows | median wall | system CPU |
|---|---|---|
| s90 (peak 1.2 GB) | 4.7–6.8 s | 5–9 s |
| another c0 (peak 1.7 GB) | 3.9–4.3 s | ≈ 2.3 s |

- **The cause:** this VM hands freed memory back to the host. A process that needs more
  memory than its predecessor freed pays for faults on memory the host has taken back.
- **The fix:** touching and freeing 3 GB before each run (`primer.py`) evens it out: c0
  takes 3.5–4.4 s after anything.
- **The first comparisons were biased by it.** Their variant order was not shuffled:
  `shuf --random-source` read the same bytes every round. So one variant always ran first,
  and it paid for this:
  - #294 itself in `logs/prototype_fixed_order_12_rounds.md`;
  - the in-place build in `logs/in_place_fixed_order.md`.
- **What survives from them:** user-CPU differences, which barely move with memory history.
- **The rotated-order run** (`logs/prototype_rotated_19_rounds.md`) is unbiased. Each
  variant led its chunk block in only 2 of 19 rounds.

## Where the write's time goes, by owner

**Method** (`owner_lib.py`, `buckets.py`, `stages.py`):
- The gdb samples of 2026-10-22-level-waves on #294, reclassified.
- Allocator and memcpy frames are charged to the innermost frame of parquet, arrow or
  genson code that called them.
- The stage is taken from the source file of the innermost genson frame.

**Write samples, c0 and s90** (`logs/write_owners.txt`):

| owner | c0 | s90 |
|---|---|---|
| level and dictionary-index RLE / bit-packing | 16% | 19% |
| byte-array dictionary indices, `Vec<u64>` growth (99% allocator) | 15% | 23% |
| level `append_run` (72–84% allocator) | 11% | 13% |
| list and struct level traversal | 9% | 6% |
| zstd | 9% | 6% |
| dictionary interning | 7% | 6% |
| compression output buffer (`reserve(compress_bound)`) | 6% | 4% |
| `write_leaf` (non-null indices, leaf levels) | 5% | 6% |

**Two redundancies stand out:**
- **Repetition levels** (`logs/c0_claims_rep_groups.txt`):
  - 93 of c0's 94 leaves are repeated, and fall into 9 groups by their innermost list.
  - Every leaf in a group gets the same repetition levels, yet each is computed on its own:
    71.0M entries where 6.8M would do.
- **Dictionary indices:**
  - They are buffered as `u64` until a page ends, and c0's columns hold 13.6M of them
    (109 MB) at once.
  - Writing 300 MB into fresh memory costs 0.5 s here against 0.004 s for reused memory,
    and a `Vec<u64>` grown by doubling to 75 MB costs 15 times one of exact capacity
    (`fresh_bytes.rs`, `logs/fresh_bytes.txt`).

## arrow-rs changes, prototyped

**The build:** a patched copy of parquet 60.0.0 under `[patch.crates-io]`, with runtime
toggles. Neither the copy nor the patch went into genson.

**The four changes:**
- **sharerep:** the first leaf under each list computes the repetition levels, and the
  others share its buffer (an Arc'd or ScalarBuffer'd `LevelData` variant). The column
  writers read them as before.
- **dict32:** `u32` byte-array dictionary indices. A dictionary page's size is an `i32` and
  each value takes at least 4 bytes, so a dictionary holds fewer than 2^29 values.
- **pagebuf:** page and compression buffers allocated at their final size.
- **levres:** level buffers reserved to the leaf array's length.

**Correctness:**
- Every output of `process()` is byte-identical with each change alone and with all four:
  7 tables, c0 and s90. The same holds for sharerep and dict32 without toggles, on all 11
  chunks.
- A clean version of sharerep and dict32 passes parquet's own tests: the arrow-writer tests
  plus a new one comparing shared and per-leaf levels on random nested arrays.

**Primed and shuffled, paired against the same binary with the changes off**
(`logs/primed_c0_s90_12_rounds.md`):

| change | c0 wall | c0 user CPU | c0 write stage | s90 wall | s90 user CPU |
|---|---|---|---|---|---|
| sharerep and dict32, without toggles | −0.35 s (−9%) | −0.77 s | −0.19 s (−21%) | −0.08 s (−6%) | −0.20 s |
| sharerep (toggled) | −0.26 s (−7%) | −0.53 s | −0.17 s | −0.04 s (−3%) | −0.15 s |
| dict32 (toggled) | −0.17 s | ≈ 0 | ≈ 0 | +0.13 s | ≈ 0 |
| all four (toggled) | −0.18 s | −0.49 s | −0.19 s | +0.08 s | ≈ 0 |

**Readings:**
- **Toggle cost:** the toggled builds check their toggles at run time, so the build
  without toggles shows more of the gain.
- **dict32's gain is fault cost:** it shows when the memory history is uneven. Unprimed and
  rotated it was −4% on c0 (`logs/prototype_rotated_19_rounds.md`); primed, it is close
  to nothing.
- **pagebuf and levres:** within noise on their own.

**All 11 chunks** (`logs/primed_series_11_chunks.md`), for sharerep and dict32 without
toggles:
- The write stage is faster on 10 of the 11 chunks: −9% to −36%, with c0 −22% and s90 −21%.
- Peak RSS is 3–100 MB lower everywhere.
- Summed over the chunks, `process()` is −9.7%, but the walls of that round were noisy.

**Unprimed but shuffled** (`logs/unprimed_series_c0_s90_15_rounds.md`):
- User CPU −0.43 s (c0) and −0.26 s (s90); write stage −31% (c0).
- The wall differences are lost in the memory-history spread: up to 3.4 s of system CPU on
  c0.

**What it would take, by route:**
- **Changing arrow-rs's internals:** sharerep is about 120 lines in `levels.rs`, dict32 a
  few in `byte_array.rs`. Both keep every encoder as it is.
- **Arrow-rs's public API:** sharerep could be had without changing arrow-rs, through
  `get_column_writer` and `write_batch` with genson-computed levels. But:
  - genson would compute every leaf's levels itself (arrow-rs's `levels.rs` is about 1,200
    lines);
  - its string columns would go through the generic `ByteArray` encoder;
  - its gain is bounded by sharerep's.
- **Out of reach of any public API:** dict32, pagebuf and levres.
- **Configuration (smaller data pages):** the one public lever on index buffering, measured
  and discarded.
  - Gain: up to −0.2 s on the write.
  - Cost: +3.5% (c0) and +1.6% (s90) on the claims file, and a different page layout for
    four tables.

## Genson's stages

**Busy samples per stage on #294** (`logs/stage_owners.txt`; their shares of CPU, inflated
for faults by the sampling):

| stage | c0 | s90 | its largest owners |
|---|---|---|---|
| write | 35% | 46% | arrow-rs, above |
| decode | 21% | 18% | Arrow string builders growing (`append_value`, `append_nulls`, `pad`): half the stage, 85–97% in the allocator |
| extract | 17% | 15% | the slim row's `String::with_capacity` and copy (`scan.rs:88`, `:94`): about half, all allocator |
| infer | 11% | 6% | key interning and row states (`interned.rs`), compute |
| read | 9% | 10% | arrow-rs's string reader growing its buffers |
| conform | 7% | 6% | `cast` of string columns |

### Extraction's slim rows written over the read buffer: discarded

**The change** (branch `perf/extract-in-place`, kept unpushed):
- `StringColumn::into_text` takes over the read arrays' text buffers. `Buffer::into_mutable`
  succeeds where nothing else holds them, and copies otherwise.
- `extract_invariants_in_place` writes each scanned row's slim text over the row's own
  bytes.
  - A cut ends at a JSON token, and each kept part moves back over the cuts before it.
  - The removed fields' texts are copied into the lookup first, as they already were.

**Correctness:**
- It matched `extract_invariants_from` byte for byte, with the same lookup and the same
  errors, on 3,000 random batches.
- Its outputs were byte-identical on all 11 chunks.

**Time:**
- Extraction got faster: c0 −0.12 s, s90 −0.035 s.
- Primed, `process()` did not: c0 −0.13 s (interval −0.34 to +0.06), s90 +0.07 s.
- The timeline (`logs/in_place_timeline.txt`) shows why. #294 frees the read buffer once the
  slim rows exist, and decoding's growing builders reuse that memory. In place, the buffer
  stays, and decoding faults fresh pages: 10–11k faults against 3–5k.

### Process per chunk, or one process for them all

**Comparison** (`reuse.py`, `logs/reused_vs_fresh_process.txt`): running the 11 chunks in
one process instead of one each, as wikidata-pq's `pool.py` does:

| | `process()` total | imports |
|---|---|---|
| one process | 12.1–14.3 s | 0.4 s once |
| one each | 13.3–14.5 s | about 4 s |

- **Large chunks after small ones** fault less: s90 1.7–2.1 s against 2.6–3.2 s.
- **RSS does not climb:** mimalloc returns freed memory, so it falls back after each chunk
  (s90 1,231–1,309 MB, then r90 266–438 MB).
- **Why pool.py spawns per chunk** is to stop RSS climbing. That choice, and imports, are
  outside this entry.

## Left open

- **The arrow-rs changes (sharerep, dict32):** contributing them is a decision outside
  genson. Doing it in genson through the public API is the costly alternative.
- **Decoding's builders:** their growth is the largest allocation genson owns. Pre-sizing
  them is the next genson-side experiment.
- **wikidata-pq's chunk-per-process pool on this VM:** see above.
