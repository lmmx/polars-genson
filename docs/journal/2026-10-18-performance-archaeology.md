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

### wikidata-pq's ingestion and the `required` finding

- wikidata-pq (5ad5f96) `normalise_claims_direct` calls `normalise_from_parquet` with
  `CLAIMS_INFERENCE_OPTIONS` (the options of `WIKIDATA_ARGS` above), `max_builders=100`,
  `typed=True`, `keep_columns=["id"]`, `extract_invariants=LABEL_INVARIANTS` and
  `prune=QUARANTINE_FIELDS`, then scans the typed file conformed to `claims_schema`
  (`missing_struct_fields="insert"`, `extra_struct_fields="ignore"`); `process()` compares
  `schema_to_dict(claims_schema)` with the Polars schema of the typed file (DeepDiff,
  `is_acceptable_diff`) and halts on a mismatch (src/wikidata/process.py:317-370, 507-517).
  Release 20260928 ran with polars-genson 0.9.6 (its uv.lock;
  docs/journal/2026-10-03-release-20260928-run.md)
- `repro_wpq_required.py` runs that function and that check in wikidata-pq's locked
  environment (polars-genson 0.9.6, Polars 1.44.2) on rows200 with ids
  (logs/e15_wpq_required.txt): the check passes; the stored `genson_avro_schema` makes
  `mainsnak.datavalue` and 18 of its fields non-nullable; every Arrow field of the typed
  file is nullable (`avro_record_fields` makes them so); Polars dtypes carry no
  nullability, so the compared schemas cannot express `required`
- `normalise_inner` normalises an absent field against a non-nullable record as a record
  of null fields: in the conformed claims, all 49 mainsnaks and 41 qualifier snaks without
  a `datavalue` hold a struct of nulls, and absent mainsnak `precision`, `latitude` and
  `longitude` are structs of nulls (16,389, 16,470, 16,470) where in qualifiers and
  references they are null; after `claims_base` (the published shape) 0 of 16,623
  statements have a null `datavalue`
- rows200 in 10 chunks of 20 rows (0.9.6): `datavalue` non-nullable in all 10; mainsnak
  `latitude`, in all 10 schemas, non-nullable in 5 and nullable in 5; qualifier
  `precision` non-nullable in 2 — the same absence stored as a null or as a struct of
  nulls depending on the chunk
- `check_published.py` on permutans/wikidata-claims at 39e6837b (release 20260928), row
  groups 0 and 45 of parts 00, 43 and 85 (949,170 rows, logs/e16_published.txt): 1,676 of
  1,676 snaks with a `snaktype` other than `value` have a non-null `datavalue` whose fields
  are all null; absent `precision`, `latitude` and `longitude` are held both ways within
  one row group (part 00, row group 0: `latitude` null 122,774 times, a struct of nulls
  35,421 times)
- master (polars-genson-py 1.0.2 at 36198dd, Polars 2.0.0, `map_encoding="kv"`) gives the
  same conformed claims as 0.9.6 (0 of 200 rows differ, 49,566 structs of nulls in each);
  with `proto_required2.py`: `mainsnak.datavalue` and every field of its record nullable,
  the check passing with the same inferred Polars schema (sha 90492ef71f4a5c04), no
  structs of nulls, and every row different from 0.9.6's as written but none once a
  struct of nulls is read as a null (`compare_conformed.py`, logs/e17_wpq_fix.txt); 49
  statements with a null `datavalue` after `claims_base`. 49,566 = 16,389 + 16,470 +
  16,470 + 49 + 41 + 147, the last the three promoted fields in each of the 49 empty
  mainsnak `datavalue`s
- With the fix, each 20-row chunk: no structs of nulls, the check passing, its inferred
  Polars schema the same as under 0.9.6, and as many null `datavalue`s as snaks without
  one (6, 10, 5, 6, 7, 4, 4, 1, 2, 4)
- rows200 holds no object that is present and empty or all-null under `datavalue`,
  `precision`, `latitude` or `longitude`; the only such objects are 9 empty `labels` maps,
  which extraction moves to the lookup table
- In Polars 1.44.2 a null struct and a struct of nulls differ under `is_null`,
  `null_count`, `drop_nulls`, `unique`/`n_unique`, `group_by`, `eq_missing`, a join on the
  struct and `struct.json_encode`; `struct.field` gives null for both
- wikidata-pq's code reads leaves of `datavalue` (claims_labels.py: `id`, `unit`) except
  demos/timeline.py, which tests `pl.all_horizontal(pl.col("point", "start", "own").is_null())`
  and takes `pl.coalesce("own", "point", "start", "event")` over whole `datavalue`
  structs: a time value, or a P585/P580 qualifier, of unknown value would take the place
  of a later date and be listed undated; the committed outputs (London, London in French,
  Vienna) list no undated row
- A release-format chunk: `release_chunk.py` writes the entity JSON of 60 items
  (`Special:EntityData`, the dumps' format) as one chunk with wikidata-pq's
  `dump.write_chunk`, and `repro_wpq_required.py` runs it with `WIKIDATA_RELEASE` set
  (the release `claims_schema` and check) (logs/e18_release_format.txt): under 0.9.6 and
  at master `mainsnak.datavalue` and 16 of its fields are non-nullable, the 33 mainsnaks
  without a `datavalue` (of 33,168) hold a struct of nulls and 32,869 absent `precision`s
  are structs of nulls; with the fix (the branch's `unification.rs`) none of them, 33
  statements with a null `datavalue`, the check passing with the same inferred Polars
  schema in all three (sha d3272d17ab24d516), and 0 of 60 rows different from 0.9.6's
  once a struct of nulls is read as a null (32,935 structs of nulls = 32,869 + 33 + 33,
  the last the `precision` of each empty `datavalue`)
- `check_snapshot_diffs.py` walks each genson-cli snapshot the fix changes against HEAD
  and accepts only `required` entries or lists removed, a type made nullable (`"default":
  null` beside it), a record of nulls become `null`, and an Avro `"null"` made optional
  written `["null"]` (as master writes any optional field of type null): 48 snapshots, 176
  types made nullable, 33 `required` lists and 16 entries removed, 33 records of nulls,
  3 `["null"]`, nothing else (logs/e19_snapshot_check.txt)

### The small fixes together

- `proto_lazy_debug.py`, `proto_merge_scalar.py` and `proto_no_validate.py` applied
  together (logs/e11_small_combined.txt), warm wall against master: rows200 W 1.66 s
  against 2.23 s (cold 1.50 s against 3.21 s), and 1.35 s with `MIMALLOC_PURGE_DELAY=-1`;
  rows200 D 1.37 against 1.44 s (1.16 s with the purge delay); slim200 W 0.108 against
  0.115 s (0.095 s); labels within the spread of runs; the same schema hashes

### Parquet reading and writing

- `bench_parquet_io.rs` times genson-core's Parquet ends on one thread
  (logs/e13_parquet_io.txt): `read_string_column` on rows200.parquet (24 MB file, 216 MB of
  JSON) 0.28–0.43 s warm (2.4 s with a cold page cache), on slim200.parquet 0.005–0.011 s;
  `write_struct_column` of rows200's typed output 0.45–0.61 s and `write_string_column` of
  its normalised JSON 0.49–0.78 s; slim200's 0.03–0.04 s and 0.02–0.08 s
- `read_columns`, which keeps the column as Arrow (#194's item 3 proposes reading `&str`
  from the batches), takes 0.30–0.36 s against 0.35–0.54 s for `read_string_column` in the
  same processes: the `String` per row is 0.05–0.15 s of the read, the rest decoding
- `open_writer` builds `WriterProperties::builder().build()` (genson-core/src/parquet.rs:305-311),
  whose codec is UNCOMPRESSED, and nothing in the workspace sets one: rows200's normalised
  JSON is written as 212 MB from a 24 MB input, its typed output as 16 MB
- `proto_parquet_codec.py` (COMPRESSION env) on the same writes: zstd level 1 writes the
  typed output in 0.45–0.61 s against 0.46–0.61 s, 5 MB against 16 MB, and the JSON in
  0.91–0.93 s against 0.49–0.78 s, 27 MB against 212 MB; zstd level 3 4 MB and 19 MB;
  snappy and lz4 are larger and no faster than zstd 1; on slim200 zstd 1 writes 0.52 MB
  against 0.84 MB (typed) and 0.50 MB against 14.5 MB (JSON) in the same time
- Summing the stage timings for the production shape (not one process), 200 rows on one
  thread: read 0.3–0.4 s, extract 1.2–1.5 s, infer 0.3 CPU-s, normalise and typed decode
  0.5–0.7 s, write 0.03–0.04 s: 2.3–2.9 s, of which extract, normalise and decode 70–75%
  and the Parquet read 13–15%

### Taking record fields in normalisation (#72)

- `proto_norm_move.py` takes fields only without `prune`; wikidata-pq passes `prune`
  (`QUARANTINE_FIELDS`), so three designs were measured against 29bb476 with
  `bench_normalise.rs` (`PRUNE=` added, and a count of pruned values) and checked with
  `diff_prune.rs` (logs/e20_normalise_take.txt)
- Taking fields and, with `prune`, keeping a copy of each row to normalise again where
  anything is pruned: no row of rows200 or slim200 prunes anything, and the copy costs
  about what taking saves (rows200 with `prune` 5.0–6.0 s against 5.9–6.4 s)
- Deciding first whether a record is pruned (a read-only look into its record-typed
  fields: only records are pruned whole, arrays and maps drop their pruned items) and then
  taking every field: the same outputs and pruned values as copying on 4,000 random cases
  (1,318 values pruned, the same hash), rows200 without `prune` 4.4–5.2 s against
  6.7–9.0 s, but slim200 with `prune` 0.51–0.62 s against 0.27–0.43 s: on rows with the
  label maps extracted the look costs more than the copies
- Taken: fields are taken without `prune` and copied with it, as before: the same hash on
  the 4,000 cases, slim200 with `prune` unchanged (0.30–0.40 s against 0.34–0.38 s),
  without it 0.26–0.32 s against 0.35–1.13 s, rows200 without `prune` 4.9–7.3 s against
  5.1–10.0 s (the VM was noisy); callgrind over the whole bench process (parse, normalise,
  serialise): rows20 3.978G against 4.497G instructions (−11.5%), slim200 2.990G against
  3.227G (−7.4%)
- `random_document` keys extend their parent's key by a letter: with keys reused at every
  depth, genson's Avro schema names two records of different fields alike (a record is
  named after its key) and `normalise_inner`'s union branch panics on `unwrap` of the
  first non-null branch of `["null"]`; that happens on master too (50 of 4,000 random
  cases even with the path keys, the same 50 in every build), and is left as it is

### Nested parallel fold of long arrays

- `ListStrategy::add_object` folds an array of 10 or more items with rayon inside a worker
  of the parallel build (genson-core/src/genson_rs/strategy/array.rs:95-130); rows200 has
  120 such arrays (3,742 items) among 32,523, labels none, narrow200k none
- `proto_list_seq.py` adds every array item by item: warm totals within the spread of
  master's on rows200 W (1.80–2.33 s against 2.09–2.31 s), rows200 D (1.28–1.66 s against
  1.52–1.54 s), labels and narrow200k, with the same output hashes (logs/e14_list_seq.txt)

### The stack's inference layers

- `bench_claims` at #232 (1a2b591), #233 (e4044c7) and #234 (eca2fb8), 3 alternated
  processes of 4 runs each, the same schema hash on every corpus in all three
  (logs/e21_stack_inference.txt), median warm wall / CPU: rows200 W 2.137/7.04,
  1.941/6.82, 2.029/6.47 s; rows200 D 1.504/4.93, 1.423/4.85, 1.285/4.32 s; slim200 W
  0.108/0.31, 0.089/0.28, 0.102/0.30 s; labels D 0.142/0.49, 0.143/0.48, 0.125/0.42 s;
  rows200 W `rewrite_objects` 0.473, 0.325, 0.343 s and `chunk.merge` 0.470, 0.448,
  0.372 s
- At #237 (the stack's top) wikidata-pq's path gives the conformed claims of #232 alone,
  row for row as written, on rows200 and on the release-format chunk, the schema check
  passing with the same inferred Polars schema; the typed temporary file is 535 KB
  where it was 852 KB; polars-genson-py's tests pass (262, 3 skipped)

## Measured and not kept

- `proto_norm_typecheck.py` compares the schema's `type` with `as_str` instead of building
  a `Value::String` per comparison: 4.486G against 4.490G instructions (20 rows); the
  wall-time differences of single runs are within the run-to-run spread
- `bench_arrow_json.rs` (arrow-json decoding the raw rows into Arrow with arrow-json's own
  inferred schema, as a bound on one pass from JSON text to Arrow) stops at schema
  inference: "Incompatible type found during schema inference", the `datavalue` that is an
  object in some snaks and a string in others
- Key interning: issue #133 records an interning attempt with lasso that was reverted
- `proto_list_seq.py`: no difference on these corpora (above)
- Validation by simd-json alone (#229 item 3): `proto_no_validate.py` on genson-cli
  accepts `{"a": 1} xyz` and `{"a": 1}}`, which master rejects (`trim_to_object` trims to
  the outer braces before simd-json parses); `{"a": 123456789012345678901234567890}`
  panics in both (simd-json errors after serde accepts it as a float), fixed in #235

## Missing

- No timing on a Wikidata chunk file (`chunk_*.parquet`): the corpora are rows200 (and
  slim200 from it), labels and narrow200k
- No normaliser writing to Arrow from the tape: the floor above is a walk that appends
  strings, without schema dispatch, nested builders or null buffers
- No timing of `normalise_from_parquet` end to end (Parquet read, extract, infer,
  normalise, write) in one process, or through the Python binding: the production-shape
  total above is a sum of stage timings
- Disk throughput on the machines that run wikidata-pq, which decides what the
  uncompressed output costs in wall time
