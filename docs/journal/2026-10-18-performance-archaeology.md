# 2026-10-18 Performance archaeology: where genson-core's inference time goes

A survey of genson-core's inference cost, phase by phase, at master 36198dd (after #226).
Scripts, prototype patches and logs are in `2026-10-18-performance-archaeology/` next to
this entry; `portfolio.md` there ranks the opportunities found. No production code is
changed by this work.

## Current State

### Method

- Measurements are release builds of genson-core/examples/bench_claims.rs on a 4-core
  Intel Xeon @ 2.10GHz VM, patched by `acct_patch.py` to print each phase of
  `infer_json_schema_from_strings` (wall, user and system CPU, minor page faults) and the
  per-string stages of the parallel path summed across threads; `build_harness.sh` builds
  each variant in its own worktree and target directory
- `acct_patch.py` without `--no-count` also counts allocations and requested bytes per
  phase; the counting (a shared atomic per allocation) triples wall time on 4 threads, so
  timings come from `--no-count` builds and allocation counts from counting builds
- `run_acct.py` runs bench_claims 5 times in one process: "cold" is the first run, "warm"
  the median of runs 2–5
- Corpora: rows200 (`bench/claims_rows.py 200`, 216.8 MB, Wikidata claims), labels (the
  `labels` column of bench/data/labels.parquet, 1,000 rows), narrow200k (200,000
  synthetic rows of six fields, `../2026-10-18-schema-hash-and-learned-indexes/corpora.py`);
  `W` is bench_claims with `WIKIDATA=1` (`map_threshold=0`, `unify_maps`,
  `wrap_root="claims"`), `D` default options, `max_builders=100` unless stated

### Phases at master (warm, logs/wave1_time.txt, logs/wave1_count.txt)

| phase | rows200 W | rows200 D | labels D | narrow200k D |
|---|---|---|---|---|
| total wall | 1.81–2.28 s | 1.36–1.44 s | 0.146 s | 0.88–0.95 s |
| chunk parallel (build, hash per string) wall | 0.80–1.10 s | 0.68–0.75 s | 0.036 s | 0.49–0.52 s |
| chunk merge (`add_builders`) wall | 0.40–0.50 s | 0.44–0.49 s | 0.106 s | 0.000 s |
| `to_schema` of the merged builder | 0.14–0.17 s (cold 0.53–0.60 s) | 0.11 s | 0.001 s | 0 |
| `rewrite_objects` | 0.41–0.50 s | 0.04 s | 0.000 s | 0 |
| allocations per run | 49.8M (13.8 GB) | 31.6M (12.0 GB) | 3.3M (0.93 GB) | 10.7M (1.6 GB) |

- On rows200 W the chunk merge makes 16.2M allocations (6.76 GB), the per-string build
  12.5M (4.41 GB), `rewrite_objects` 18.6M (1.87 GB) and `to_schema` 1.7M (0.25 GB) — the
  stage split of 2026-10-18-schema-hash-and-learned-indexes.md timed the per-string build
  alone
- Allocations per run are 63 bytes per input byte on rows200 W and 64 on narrow200k
- On labels the chunk merge is 72% of wall time and 0.357 user-s against 0.086 thread-s of
  building the strings it merges
- On narrow200k 0.41 s of the 0.9 s run falls outside both the parallel phase and the
  merge, in the loop over 2,000 chunks of 100 strings, with 0.45–0.56 s of system CPU and
  under 200 page faults

### Kernel time from memory churn

- rows200 W spends 0.87–1.94 s of system CPU per warm run (27–34% of all CPU), with
  126K–168K minor page faults per run, nearly all in the chunk parallel phase
- `MIMALLOC_PURGE_DELAY=-1` (mimalloc keeps freed pages instead of returning them to the
  OS) takes rows200 W from 1.99 s to 1.60 s warm wall (3.35 s to 2.67 s cold), system CPU
  from 0.87 s to 0.09 s and page faults from 166,900 to 8 per run, with peak RSS 1,350 MB
  against 1,223 MB; rows200 D goes from 1.44 s to 1.27 s and narrow200k does not change
  (logs/e1_purge.txt) — the 2026-10-08 entry records the same setting at da646ea (3.15 s
  against 4.26 s) without the page-fault count
- Each chunk's 100 per-string builders are alive together until the merge; peak RSS on
  rows200 W is 825, 1,204 and 1,750 MB at `max_builders` 25, 100 and 200, about 5.3 MB of
  builder per 1.1 MB input string, with total time unchanged (1.95–2.00 s,
  logs/e2_chunk.txt)

### Chunks of a fixed number of strings

- narrow200k with `max_builders` 100, 1,000, 10,000 and 200,000 takes 0.95, 0.43, 0.27
  and 0.42 s warm, with 0.45, 0.05, 0.005 and 0.14 s of system CPU and 48, 58, 116 and
  1,033 MB peak RSS (logs/e2_chunk.txt)

### Folding strings into one builder per worker (`proto_fold.py`)

- `proto_fold.py` replaces the chunk loop with one `try_fold` over all strings, each
  worker adding its documents to its own builder with `add_object`, the workers' builders
  merged at the end; no per-string builder, hash or chunk
- narrow200k: 0.082 s against 0.884 s (10.8×), labels: 0.039 s against 0.146 s (3.7×),
  both with the same schema hash as master (logs/e4_fold.txt)
- rows200 W: 2.56 s against 1.81 s, same schema hash; rows200 D: 2.18 s against 1.36 s,
  with two different schema hashes over 5 runs, neither master's
- On one thread rows200 D takes 2.98 s folded against 4.01 s at master (cold 3.56 s
  against 7.78 s), with 153K page faults against 696K, and a schema hash of its own
  (logs/e4_fold_1thread.txt) — the fold does less work and scales 1.37× from 1 to 4
  threads, where master scales 2.9×

### Record interpretation against map interpretation

- The builder holds every object key as a field until `rewrite_objects`: rows200's row
  builders hold 13,136 nodes per row and the merged builder 213,685 node positions, where
  the same rows with the 11 map paths of the W schema collapsed hold 53 per row and 81 in
  the union (`paths.py`); the W schema has 88 nodes and the D schema 18,937
- `collapse.py` rewrites rows200 so that every key at those 11 map paths is `"*"`
  (duplicate keys, which simd-json's tape keeps in order), so the unchanged builder merges
  all of a map's values into one child as it reads them
- bench_claims W on the collapsed corpus takes 0.29 s warm and cold against 2.08 s warm
  and 3.38 s cold on rows200, with merge 0.005 s, `rewrite_objects` 0.001 s, 5.7K page
  faults against 161K and 462 MB peak RSS against 1,203 MB (logs/e5_collapsed.txt)
- The collapsed corpus gives a different W schema (3,331 bytes against 3,866): `required`
  sets, nullable types and the type of `precision` differ (next section)

### Map unification and `required`

- `unify_record_schemas` marks a field required, and keeps it non-nullable, when the field
  is in the `properties` of every input schema, whatever the inputs' own `required`
  (genson-core/src/schema/map_inference/unification.rs:533-565) — an input that is itself
  a unified schema holds every field seen so far, so a field optional in every row becomes
  required again when unified with it in the pairwise fold or the parallel halves
- `check_required.py` counts, for rows200, the objects missing a key that the W schema at
  master requires: 10,040 of 64,293, at 4 of 7 paths — 7,337 of 7,337 mainsnak
  `datavalue` objects miss some of its 20 required keys (`id`, `labels`, `amount`,
  `latitude`, …, which no one datavalue holds together), and 49 of 16,623 mainsnaks have
  no `datavalue`, which the schema requires
- `proto_required.py` makes a field required only when every input requires it, and
  nullable otherwise; its W schema has 0 violations of 51,220, as has the W schema of the
  collapsed corpus
- The W schemas of `proto_required.py` and of the collapsed corpus have equal `required`
  sets everywhere; they differ in 12 places, all of two kinds: `precision`, `integer` for
  some keys and `number` for others, unifies to an object of `precision__integer` and
  `precision__number` where the build gives `number`, and a field absent from some objects
  is a nullable type in the unified schema and a non-required, non-nullable one in the
  built schema

### Per-string preparation

- `prepare_json_bytes` checks UTF-8 with `std::str::from_utf8` on input that is already a
  `&str`, and parses each document with serde_json (`IgnoredAny`) to validate it before
  simd-json parses it again (genson-core/src/schema.rs:159-216); the stage takes 0.38–0.51
  thread-s per warm run on rows200 W

## Missing

- No breakdown of the chunk merge by function
- No measurement under wikidata-pq's own claims options (`--force-type mainsnak:record`,
  `--force-scalar-promotion`, `--no-unify qualifiers`), which bench_claims does not take
- No run of `proto_required.py` against the genson-cli snapshot suites
