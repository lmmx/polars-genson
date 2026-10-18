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

- genson-cli on rows200 as a Parquet column (`--pq-column claims`, one string per row)
  with wikidata-pq's claims options (`WIKIDATA_ARGS` of genson-cli/tests/wikidata_config.rs
  and `--max-builders 100`) gives the same 10,040 of 64,293 violations at master and 0
  with `proto_required.py`
- Under those options the Avro schema at master (`--avro`) makes all 20 fields of the
  mainsnak `datavalue` record and the `datavalue` field of `mainsnak` non-nullable; with
  `proto_required.py` none of the 20 and not `datavalue` (logs/production_options.txt)
- `normalise_inner` fills a record field missing from its input with `null`
  (`field_value`, genson-core/src/normalise.rs:108-114), so rows normalised against the
  master schema hold nulls in fields that schema declares non-nullable
- The W schema inferred from the first 1, 2, 5, 10, 20 or 50 rows of rows200 has the same
  11 map paths as the one inferred from all 200, and no others (`map_stability.py`)
- Issue #50 (2025-09-06) suspects that merging schemas in map inference marks every field
  required; issue #110 records `__integer`/`__number` promotion as the chosen form for
  mixed integer and float values

### Instruction counts (callgrind, one thread)

- Release builds keep no symbols (`strip = "symbols"`, Cargo.toml); the profiles below
  use builds with `CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`
  and `RAYON_NUM_THREADS=1`
- labels D (1.95G instructions): `absorb_all` (the chunk merge) 48.5% inclusive, within
  which `BasicSchemaStrategy::to_schema` 10.1% (a scalar merges through
  `strategy.add_schema(&other.to_schema())`, genson-core/src/genson_rs/strategy/mod.rs) and
  `absorb_keywords_and_required` 12.2%; `schema_hash` 17.0%, of which `Xxh64::update` 8.6%
  over 3.5M writes; `validate_json` 4.7%; simd-json parsing 6.2%
- rows200, first 40 rows, D (7.09G): chunk merge 25.8%, build (`ObjectStrategy::add_object`)
  19.2%, simd-json 15.1%, `validate_json` 9.9%, `schema_hash` 8.6%, final `to_schema` 6.8%,
  dropping `Value`s 4.6%, `rewrite_objects` 3.2%
- rows200, first 40 rows, W (13.66G): `check_unifiable_schemas` 46.5% inclusive (on rayon
  jobs, outside `rewrite_objects`' own 8.3%), dropping `Value`s 8.3%, `unified_values`
  7.3%, merge 13.4%, build 10.0%, simd-json 7.9%
- `serde_json` serialisation into `Vec<u8>` takes 1.76G instructions (12.9%) of the W run,
  from 26,531 top-level calls: `check_unifiable_schemas` passes
  `serde_json::to_string(schema)` for every input schema to `debug_verbose!`
  (genson-core/src/schema/map_inference/unification.rs:686-695), and the macro expands to
  `$cfg.debug_verbose(format_args!(...))`, which evaluates its arguments before
  `debug_verbose` checks `self.debug` (genson-core/src/schema/core.rs:82-95, 164-175); the
  same holds at unification.rs:271-277 (each array's items) and 784-791
- `proto_lazy_debug.py` checks `$cfg.debug` in both macros before building the arguments:
  `rewrite_objects` on rows200 W takes 0.31–0.34 s against 0.43–0.44 s warm (user 0.76–0.80 s
  against 0.98–0.99 s), with the same schema hash (logs/e6_lazy_debug.txt)

### Per-string preparation

- `prepare_json_bytes` checks UTF-8 with `std::str::from_utf8` on input that is already a
  `&str`, and parses each document with serde_json (`IgnoredAny`) to validate it before
  simd-json parses it again (genson-core/src/schema.rs:159-216); the stage takes 0.38–0.51
  thread-s per warm run on rows200 W
- `proto_no_validate.py` (upper bound: no serde validation, simd-json the only parse) takes
  the stage from 0.30 to 0.03 thread-s on rows200 D and from 0.47 to 0.11 on rows200 W
  (the rest is the `wrap_root` copy), total wall 1.50 → 1.38 s (D), 2.09 → 1.66 s (W) and
  0.161 → 0.130 s (labels), with the same schema hashes (logs/e8_merge_scalar_validate.txt)
  — issue #194 item 4c proposes letting simd-json validate and falling back to serde on
  failure only

### Merge of a scalar

- `proto_merge_scalar.py` merges a scalar with no keywords into one of its kind without
  `to_schema()` (a no-op, but for `number` over `integer`): chunk merge 0.113 → 0.090 s
  (labels), 0.513 → 0.391 s (rows200 D), 0.444 → 0.354 s (rows200 W), same schema hashes
  (logs/e8_merge_scalar_validate.txt)

### Folding, corrected

- The first version of `proto_fold.py` gave each worker's builder the AUTO `$schema`
  handling, so every merge of workers' builders after the first went through
  `SchemaBuilder::add_builder`'s `self.add_schema(other.to_schema())` branch
  (genson-core/src/genson_rs/builder.rs:97-108), which drops an empty `required` set —
  the rows200 D difference of the first version was one `required` list
  (`P832/items/qualifiers`, `["P828", "P837"]`) that 1 of its 5 objects violates; the
  production path merges with `add_builders`, which does not take that branch
- With workers' builders that have no `$schema` (NULL, as
  `build_multi_json_objects_schema`'s), the fold gives master's schema hash on rows200 W
  and D (3 of 3 runs on 4 threads each), narrow200k and the collapsed corpus; on one thread
  rows200 D and on 4 threads labels give equal JSON values with keywords in another order
  (`type, properties, required` against master's `properties, required, type`)
- Warm wall, fold against master (logs/e4_fold_v2.txt): narrow200k 0.110 s against 0.941 s,
  labels 0.032 s against 0.143 s, rows200 W 2.08 s against 2.03 s (CPU 5.42 against 6.67 s,
  page faults 71K against 148K per run, cold 2.54 against 3.23 s), rows200 D 1.70 against
  1.55 s (CPU 4.37 against 5.23 s), collapsed corpus 0.262 against 0.296 s
- `absorb_keywords_and_required` records each merged object's keyword order as placeholder
  extra keywords so that the output's keyword order follows the `add_schema(to_schema())`
  semantics (genson-core/src/genson_rs/strategy/object.rs:464-500); it is 12.2% of labels'
  instructions, and the 2026-10-08 entry measured 28 genson-cli snapshots failing under a
  canonical keyword order

### Normalisation

- polars-genson-py's `normalise_from_parquet` parses each row with `serde_json::from_str`,
  normalises it with `normalise_values_pruned` into a new `Value`, and then either
  serialises it (`serde_json::to_string`) or, with `typed`, converts all rows with
  `values_to_struct_array` (polars-genson-py/src/parquet_io.rs:309-412)
- `bench_normalise.rs` runs that row loop on one thread against the Avro schema genson-cli
  infers for rows200 under wikidata-pq's options: parse 1.0–1.6 s, normalise 6.5–11.7 s,
  serialise 0.7–1.7 s, and `values_to_struct_array` 4.1 s, against 4.7–6.7 CPU-s for
  inferring the schema on 4 threads (logs/e7_normalise.txt)
- callgrind on its first 20 rows: `normalise_inner` 68.6% inclusive, of which
  `IndexMap::insert_full` (building the output objects, `serde_json`'s map under
  `preserve_order`) 31.1%, SipHash of keys 12.1%, and dropping `Value`s 12.5%
  (logs/callgrind_norm20_*.txt)
- The record branch reads each field with `field_value`, `m.get(name).cloned()`
  (genson-core/src/normalise.rs:108-114), copying the field's subtree out of an input it
  owns; issue #72 proposes normalising in place
- `proto_norm_move.py` takes each field out of the owned input (`Map::remove`) when no
  field is pruned: 3.97G against 4.49G instructions for the 20-row run, the normalise
  stage 4.4 s against 7.4 s median wall on 200 rows, with the same output hash
  (logs/e7_normalise_ir.txt, logs/e7_normalise.txt)

- `bench_norm_floor.rs` parses each rows200 row with simd-json's tape and walks it once,
  appending every string (keys and values, 9,101,382 strings, 201 MB) to an Arrow
  `StringBuilder`: 0.54–0.59 s parse and 0.19–0.21 s walk on one thread
  (logs/e9_norm_floor.txt); rows200 holds 19,432 values that are not strings
- arrow-json, given the Arrow schema `avro_record_fields` derives from the production Avro
  schema (native maps), decodes rows200 normalised by `bench_normalise.rs` (`MAPPING=1`,
  142 MB of NDJSON) into typed Arrow in 0.52–0.63 s on one thread (`bench_arrow_json.rs`,
  logs/e9_norm_floor.txt)

### The production shape: label maps extracted

- `normalise_from_parquet` with `extract_invariants` (and a `lookup_output_path`) runs
  `extract_invariants` on the rows before inference and normalisation
  (polars-genson-py/src/parquet_io.rs:229-243); `extract_invariants` parses each row into a
  `Value`, removes each invariant field with `shift_remove`, compares every repeated
  `(field, key)` value with the stored one (`Value` equality), and serialises the slim row
  again (genson-core/src/extract.rs)
- `bench_extract.rs` with wikidata-pq's spec (`labels`→`id`, `property-labels`→`property`,
  `unit-labels`→`unit`) on rows200: 1.17–1.46 s on one thread, 0.42–0.68 s on four, 216 MB
  in, 4 MB of slim rows and 1,924 lookup entries out (logs/e10_extract.txt); callgrind on
  its first 20 rows: `Lookup::insert` 16.4% inclusive, of which `Value` equality 11.9%,
  `strip` 14.5%, dropping `Value`s 6.0%, the rest the serde parse
- `slim.py` drops those three fields from rows200 (slim200, 4.6 MB): the record
  interpretation holds 812 nodes per row and 12,508 merged against 40 and 65 for the map
  interpretation, and the W schema has the same 10,040 `required` violations
- slim200 through genson-cli with wikidata-pq's options takes 0.11–0.20 s wall (user
  0.29–0.31 s), and its normalisation against that schema on one thread 0.03 s parse,
  0.31–0.56 s normalise and 0.16 s `values_to_struct_array`, the output 14 MB from 4.6 MB
  (a `datavalue` record of 20 fields per snak)
- bench_claims W on slim200 (warm 0.104 s): chunk merge 0.040 s, parallel build 0.027 s,
  `rewrite_objects` 0.025 s, `to_schema` 0.007 s; 2.58M allocations of 595 MB, 129 bytes
  per input byte
- The fold prototype takes 0.127 s against 0.097–0.106 s on slim200 W, with the same schema
  hash (logs/e12_fold_slim.txt)
- The W schema of the first 1, 2 and 3 rows of genson-cli/tests/data/claims_fixture_x30.jsonl
  has 9 of the 11 map paths of all 30 rows, and that of the first 5, 10 and 20 rows all 11,
  with no others (logs/map_stability_x30.txt)

### Snapshot impact of the `required` change

- `proto_required.py` changes 67 genson-cli snapshots: claims_fixtures 40, wikidata_config
  20, unify_scalar 4, map_of_unified_records 3 (logs/req_cli_tests_all.txt) — `required`
  lists, nullable types, and the order of fields, as fields required by every input are
  listed first
- `proto_required2.py` keeps master's field order (fields in the `properties` of every
  input first) and takes only `required` and nullability from the inputs: 48 snapshots
  change, claims_fixtures 24, wikidata_config 20, unify_scalar 4
  (logs/req2_cli_tests_all.txt); 4 of them are normalised output, where an absent promoted
  record (`latitude`, `precision`) is `null` instead of a record of nulls

### The small fixes together

- `proto_lazy_debug.py`, `proto_merge_scalar.py` and `proto_no_validate.py` applied
  together (logs/e11_small_combined.txt), warm wall against master: rows200 W 1.66 s
  against 2.23 s (cold 1.50 s against 3.21 s), and 1.35 s with `MIMALLOC_PURGE_DELAY=-1`;
  rows200 D 1.37 against 1.44 s (1.16 s with the purge delay); slim200 W 0.108 against
  0.115 s (0.095 s); labels within the spread of runs; the same schema hashes

## Measured and not kept

- `proto_norm_typecheck.py` compares the schema's `type` with `as_str` instead of building
  a `Value::String` per comparison: 4.486G against 4.490G instructions (20 rows); the
  wall-time differences of single runs are within the run-to-run spread
- `bench_arrow_json.rs` (arrow-json decoding the raw rows into Arrow with arrow-json's own
  inferred schema, as a bound on one pass from JSON text to Arrow) stops at schema
  inference: "Incompatible type found during schema inference", the `datavalue` that is an
  object in some snaks and a string in others
- Key interning: issue #133 records an interning attempt with lasso that was reverted

## Missing

- No timing on a Wikidata chunk file (`chunk_*.parquet`) or of Parquet reading and writing:
  the corpora are rows200, labels and narrow200k
- No normaliser writing to Arrow from the tape: the floor above is a walk that appends
  strings, without schema dispatch, nested builders or null buffers
- No measurement of `ListStrategy::add_object`'s parallel fold over arrays of 10 or more
  items (genson-core/src/genson_rs/strategy/array.rs:95-130) inside the parallel phase
- No timing of `normalise_from_parquet` end to end (Parquet read, extract, infer,
  normalise, write) in one process
