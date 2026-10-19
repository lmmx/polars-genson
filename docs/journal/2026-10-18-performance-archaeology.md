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

### The data path on wikidata-pq's chunks

- Corpora (in the session's scratchpad, not committed): rows3000p (3,000 rows of
  `claims_rows_small.py`, `id` and `claims`, 332 MB of claims JSON, rewritten by pyarrow
  25 without dictionary encoding: plain, no offset index), rows3000m (the same rows with
  the labels, descriptions, aliases and sitelinks of the release-format chunk's 60
  well-known items repeated in turn, so those four columns are far larger than a typical
  entity's), rowsw
  (rows3000m's 2,185 distinct rows with an `entity` column, written as wikidata-pq's
  `dump.write_chunk` writes a chunk: Polars, zstd level 9; 322 MB of claims), rows200,
  and rel7k (`release_chunk_synth.py`: a release-format chunk of a main-set chunk's
  median shape, 7,128 entities in 3.96 MB, made from the 60 entity JSON files
  `release_chunk.py` fetched earlier, through `dump.write_chunk`; nothing new fetched)
- Polars dictionary-encodes a string column with repeated values: written by Polars,
  rows3000m's claims (2,185 distinct of 3,000) were dictionary-encoded, which a chunk of
  distinct entities is not, hence rowsw's distinct rows. Polars 1.44.2 (wikidata-pq's
  lock) and 2.0 write an offset index and a column index; pyarrow 25 writes neither by
  default
- `bench_wpq.py` times `normalise_claims_direct`, `bench_wpq_maps.py` the three
  `normalise_map_direct` calls and `bench_process.py` the whole of `process()` on one
  chunk (every table, the id and schema checks; `WIKIDATA_RELEASE` for a release chunk),
  each run in a fresh process, with `MAP_ENCODING=kv` (polars-genson 1.0 defaults to
  maps); master is 36198dd, and the candidates the stack's top with each change
- The projected read (#194 item 3): `read_string_column_opt` built its reader without a
  projection, so each of wikidata-pq's four `normalise_from_parquet` calls per chunk
  decoded every column, then copied each row into a `String`. Reading the one column as
  borrowed rows (logs/e23_projected_read.txt): the three map steps on rows3000m
  3.47–4.60 s against 0.71–0.79 s; on rows3000p the claims step's read 1.23 s against
  0.56 s (medians of 3)
- Extraction by scanning each row's text (#228): one pass validates the row by
  serde_json's rules for `Value` and finds the removed fields as byte ranges; a removed
  value's text is parsed only when its (field, key) is new and its text differs from the
  one held. `diff_extract.rs` against the `Value` path on four corpora
  (logs/e25_diff_extract.txt): identical lookups, slim rows that parse to the same values
  with keys in the same order, equal inferred schemas under wikidata-pq's claims options;
  all threads, rowsw 0.21–0.31 s against 0.64–0.76 s, rows3000p 0.17–0.29 s against
  0.69–0.72 s, rows200 0.15–0.25 s against 0.46–0.56 s, rel7k (no label maps) 0.010 s
  against 0.027 s. The property tests compare the two paths on 3,000 random batches and
  reject 20,000 mutations of valid rows as serde_json does (a stress run of 30,000 and
  200,000 under other seeds passed). The claims step on rows3000p fell from about 3.4 s
  to 1.65 s, peak RSS from 750 to 544 MB (logs/e24_scan_extract.txt)
- Along the way: reading `\uXXXX` with `from_utf8` and `from_str_radix` made escapes the
  scan's slowest part (a table and an 8-byte SWAR test for plain runs replaced it); slim
  rows allocated at the row's length raised peak RSS by 375 MB and slowed extraction
  (they are sized exactly); parsing every first-seen removed value in each chunk cost
  about 31,000 parses on rows3000p (values are held as text, compared as text, and
  parsed when they differ or are written); rayon's `collect` into a `Result` reported
  whichever chunk's conflict came first (each chunk's result is kept and the first in
  row order reported)
- Parallel decoding of the column (`par_read.rs`, medians of 5): rowsw one reader 0.31 s,
  4 readers over row ranges 0.12 s, 8 0.11–0.13 s, 16 0.058 s; rows200 0.28 s and
  0.044–0.056 s with 16; rows3000p, without an offset index, 0.62 s, 0.32 s with 4, and
  0.47 s and 0.81 s with 8 and 16 (each reader walks the column's page headers from the
  start). Loading the offset index or not made no difference to these. Ranges start at
  pages by the offset index, balanced by compressed bytes, or else at row groups; a file
  with neither is read by one reader. Tested against one reader on four layouts (pages in
  one and several row groups, row groups without an offset index, one row group without
  one), both string types, 0 to 200 rows and 1 to 1,000 parts; of three mutants of the
  selection, two fail the test and the third is equivalent (page starts strictly
  increase)
- The claims step on rowsw, 6 interleaved rounds (logs/e26_claims_rowsw.txt), median wall
  [range], CPU, peak RSS: master 3.40 s [2.55–5.11], 5.68 s, 754 MB; projected read and
  scan 1.83 s [1.40–2.50], 3.05 s, 591 MB; with parallel decoding 1.16 s [1.03–1.45],
  2.67 s, 626 MB. Its stages at the top: read 0.10, extract 0.23, infer 0.18,
  normalise and decode 0.16, write 0.06 s; outside `normalise_from_parquet`,
  `lookup_to_long` 0.2 s and the conforming scan 0.1 s
- The whole of `process()` on rowsw, 3 interleaved rounds (logs/e27_process_rowsw.txt):
  master 12.75 s [12.70–13.36], CPU 18.8 s; the top 6.64 s [6.52–9.11], CPU 11.9 s. The
  map steps 5.40 s against 0.53 s, the claims step 3.03 s against 1.21 s. Left as they
  were: `normalise_sitelinks` 3.5–3.7 s (polars-genson's `normalise_json` expression,
  which normalises its rows one at a time), Polars' writes of the tables 1.0 s, and
  `pl.read_parquet` of the whole chunk 0.6 s. rowsw's sitelinks are those of well-known
  items in every row (above): on rel7k, whose entities keep a few, `normalise_sitelinks`
  takes 0.1 s
- A release chunk's claims carry no label maps (`lookup_to_long`: "An official dump's
  claims carry no labels"), so on the release being run the extraction has nothing to
  remove and the claims column is small. `process()` on rel7k, 6 interleaved rounds
  (logs/e28_process_rel7k.txt): master 2.56 s [2.51–3.26], CPU 4.80 s; the top 2.48 s
  [2.37–2.63], CPU 4.42 s. The claims step 1.29 against 1.20 s, in which inference
  0.62 against 0.54 s and normalisation with the typed decode 0.34 against 0.30 s;
  Polars' writes of the tables 0.90 s. On a release chunk polars-genson's share is
  inference and normalisation, about 1 s of 2.5 s

### Inference and normalisation of release-format claims

- `bench_infer_wpq.rs` times inference on a chunk's claims column with wikidata-pq's
  claims options and prints a schema hash; `bench_normalise_wpq.rs` times the typed row
  loop of `normalise_from_parquet` (parse, `normalise_values_pruned` with key/value maps
  and wikidata-pq's prune names, `values_to_struct_array`) and hashes the rows and pruned
  values. Instruction counts are callgrind on one thread (`RAYON_NUM_THREADS=1`); a
  `--toggle-collect` counts only the thread it is toggled on, so a phase that runs on the
  rayon pool is counted by toggling a function on the worker's side
- On rel7k at #241's tip, inference is 5.60G instructions. `schema_order`,
  `absorb_keywords_and_required` and the rest of the merge of each chunk's rows 1.33G,
  building each row's schema 1.19G (of which simd-json about 0.2G), map unification in
  `rewrite_objects` 1.63G, hashing rows to skip duplicates 0.56G, `to_schema` 0.20G.
  55% of the instructions are under `serde_json::to_value` of a `Value`: `json!` turns
  each value it is given into a `Value` that way, which copies an owned one by
  serialising it, and map unification wrapped every unified record, array and map with it
- Built by moving the values in (`value::object`): inference 4.64G where it was 5.58G
  (-17%), unification 0.72G where it was 1.63G; the normalise bench as a whole (inference,
  parse, normalisation, decode) 12.75G where it was 15.19G, `apply_map_encoding` having
  copied every key/value entry the same way
- Merging a node with one strategy without reordering it (two vectors and moves of a
  few hundred bytes per node, about 8.4M allocations), and building an object's keyword
  placeholders only for keys it lacks: the merge 0.69G where it was 1.33G, inference
  4.01G where it was 4.66G
- On 4 cores (a patch timing each step, not kept) `to_schema` took 95-115 ms of about
  450 ms, on one thread, and dropping the builder's tree 20 ms. Writing the properties of
  an object of 16 or more in parallel, and dropping the tree on the rayon pool: median
  wall of runs 2-9 in a process, 3 rounds, #241's tip 0.534-0.542 s, moved values
  0.478-0.505 s, the merge 0.445-0.464 s, parallel `to_schema` 0.391-0.433 s; CPU 1.40 to
  1.15 s. The schema hash is the same at each step, and `bench_claims` gives the same
  hashes and processed counts as #241's tip on rows200 (with and without the Wikidata
  options), slim200, labels, narrow200k, rows20, the collapsed and normalised rows200 and
  p832 (a first comparison built both binaries into one target directory and compared a
  binary with itself; the second used separate directories)
- Not taken: keeping the unified schema so far when the next is equal, rather than
  cloning it, changed nothing (0.716G and 0.721G): the clones are of each field's first
  schema, borrowed from the input. Returning `left` when the two halves of
  `unify_field_schemas_parallel` are equal would skip a unification that can change the
  schema (two equal nullable arrays unify to a non-nullable one), so it would change
  output
- Normalisation read the Avro schema as JSON for every value: `type` looked up up to three
  times per object against a freshly allocated string, a record's `fields` walked three
  times, each field's `name` and `type` looked up and `prune` checked twice, all by
  SipHash. `NormalisePlan` reads the schema once into a tree of node kinds, record fields
  with their plans, the integer promoted field and the fields `prune` names: on rel7k
  normalisation is 2.19G where it was 5.41G (-60%), 0.23-0.34 s where it was 0.40-0.54 s
  on one thread, with the same rows. It is tested against the schema-reading path on
  2,000 random batches under every option (with rows of the previous batch against each
  schema) and on schemas inference does not write; seven mutants of the plan all fail
  the tests
- `process()` on rel7k, 6 interleaved rounds (logs/e29_process_rel7k.txt), median wall
  [range], CPU: master 2.553 s [2.374-3.269], 4.66 s; #237 2.486 s, 4.37 s; #241 2.369 s,
  4.19 s; with the four changes above 2.124 s [2.051-2.423], 3.45 s. The claims step
  1.227, 1.141, 1.135 and 0.857 s; inference 0.605, 0.552, 0.541 and 0.401 s;
  normalisation and the typed decode 0.288, 0.258, 0.267 and 0.127 s
- Every table `process()` writes (`compare_tables.py`) is equal at #237 and with the
  changes above, on rel7k and on rowsw: 7 and 6 tables, schema and values (pyarrow's
  `Table.equals`; Polars' `assert_frame_equal` and `DataFrame.equals` ran for over ten
  minutes on the aliases' list of structs of lists without finishing)
- `process()` on rowsw, 3 rounds (logs/e30_process_rowsw.txt): master 13.34 s, #241
  6.99 s, the changes above 6.62 s; `normalise_sitelinks` 3.2-3.5 s throughout. Of its
  83 MB of sitelinks (the well-known items' own), inference takes 0.75 s, the
  `normalise_json` expression's normalisation of one row at a time 0.9-1.9 s, and
  Polars' `json_decode` of its output 0.6-0.8 s
- The `normalise_json` expression normalising its rows in parallel (in order): on rowsw's
  sitelinks the expression without its decode takes 1.03-1.04 s where it took 1.84-1.90 s.
  `process()` (logs/e31_process_parallel_expression.txt): rel7k, 6 rounds, master
  2.561 s [2.347-2.894], CPU 4.76 s, against 2.056 s [1.973-2.544], CPU 3.46 s (-20% wall,
  -27% CPU); rowsw, 3 rounds, master 13.74 s [12.71-14.19] against 5.48 s [5.08-6.28].
  The tables are still equal to #237's on rowsw, and polars-genson-py's tests pass (262,
  3 skipped)
- Left with Polars on rel7k: writing the conformed claims (`atomic_sink_parquet`) takes
  0.89-0.91 s of the 2.12 s, where genson writes the typed claims in 0.13 s. Rewriting the
  table (14 MB in memory, 94 leaf columns) takes 0.7-1.2 s with `sink_parquet` or
  `write_parquet` under Polars 2.0 and 1.44.2 (wikidata-pq's), with or without
  statistics, and 0.27-0.41 s with `write_parquet(use_pyarrow=True)`; reading the whole
  chunk (`pl.read_parquet`) 0.23 s

### Normalisation straight into Arrow

- `ArrowPlan` (`NormalisePlan::to_arrow`, `normalise/typed.rs`) is the normalisation plan
  compiled against the Arrow fields the rows decode into. Each row is read once into a
  token tape, validated by serde_json's rules for a `Value` (whitespace, number grammar
  and range, escapes and surrogates, the 127-level limit), and the compiled plan walks
  the tape and appends to Arrow builders: no `Value` for the row, none for its normalised
  form, no JSON written for arrow-json to read back, and members the schema has no field
  for are validated but never decoded. `normalise_from_parquet`'s typed batches use it,
  each batch falling back to the `Value` path where the plan declines (`None`): the
  `entries` encoding, fields other than `avro_record_fields`' for the pruned schema (or
  none: `values_to_struct_array` fails on a struct without fields), a value in `["null"]`
  or `[]` (a panic there), a record pruned by a field after one that could panic on its
  value, a value in a column of nulls, and batches past arrow-json's limits (`i32` offsets
  of a string, list or map column; `u32` tape offsets, which arrow-json's `finish` checks
  twice, where the second check was meant for its elements)
- Instructions on rel7k's claims, one thread (`bench_typed_wpq.rs`, `ONLY=`, callgrind
  toggled on `measured`): the `Value` path 3.87G; the first tape decoder 1.38G, of which
  `can_prune` recomputed at every array item took 11%, a `String` allocated per field per
  row for the pruned values' path 10%, linear member lookups 15%, and reading the
  promoted suffix of field names 7%; compiled once (pruning flags, promoted kinds, a
  name-to-slot table per record with the next field tried first, borrowed path segments
  made `PathSegment`s only when a value is pruned) 0.79G. Allocator instructions 699M
  against 1M, `memcpy` 152M against 27M. What is left: the lexer (about a quarter), the
  plan's dispatch, null structs appended to every descendant
- Time, the `Value` path against the tape, `bench_typed_wpq.rs` (one thread; rayon's four
  with `THREADS=1`), arrays and pruned values equal in every run: rel7k 0.54-1.16 s
  against 0.06-0.10 s, four threads 0.13-0.19 s against 0.016-0.019 s; rowsw with its label
  maps extracted (2,185 rows, 6.9 MB) 0.32-0.42 s against 0.03-0.05 s, four threads 0.10 s
  against 0.011-0.012 s; rows200 and rows3000p the same way 6-21x on one thread, 7.5-11x
  on four; rowsw as it is (322 MB of claims) 11.6 s against 1.8 s
- Other rows (`bench_typed.rs`, the schema inferred from them, key/value maps): event logs
  from `gen_events.py` (100,000 rows, 58.5 MB: a payload per event type, optional and null
  fields, lists of records, integers and floats in one field, escaped and non-ASCII
  strings; `python gen_events.py 100000 events.ndjson`) 1.06-2.11 s against 0.20-0.24 s,
  four threads 4.6-5.6x; narrow records (narrow200k) 5.4-5.7x, four threads 4.1-6.2x;
  label maps (labels) 3.2-4.4x, four threads 2.4-2.9x (a map's keys are hashed for
  repeats and copied). Equal too with native
  maps, `wrap_root`, and prunes. Pruning many rows whole is slower to record, each pruned
  value being built as a `Value` from the tape: event logs pruned by `display_name` and
  `closed_at` (40,498 values, whole rows, the actor being a field of the row) 1.7-2.7 s
  against 0.94-0.99 s; the instructions are 18% above the unpruned run's, the time is system
  time (6.6 s against 3.4 s over the process) from the pruned values' memory
- Tests: the decoder against the `Value` path on 2,500 random batches of valid, invalid
  and null rows (whitespace, every number form, escapes, surrogates, repeated and escaped
  keys, promoted names) under every option and prune set, with rows wrapped, beside a
  field or not, and schemas inferred from all the rows or all but the last; at the depth
  limit; a prune after a field that panics. 21 mutants all fail the tests
  (logs/e34_typed_mutants.txt): the first pass left three alive, each a gap in the test
  (rows that do not fit their schema, a root field beside the wrapped row, depth 128),
  and closing the third found the decline too broad (a null field cannot panic unless it
  is an empty union)
- `process()` on rel7k, 6 interleaved rounds (logs/e32_process_rel7k_typed.txt), median
  wall [range], CPU, peak RSS: master 2.628 s [2.553-2.802], 4.93 s, 457 MB; #244 2.040 s
  [1.970-2.236], 3.49 s, 434 MB; with the tape 1.928 s [1.801-2.144], 2.95 s, 384 MB.
  Normalisation and the typed decode 0.326, 0.142 and 0.019 s; `normalise_claims_direct`
  1.30, 0.86 and 0.72 s wall, 3.34, 2.04 and 1.55 s CPU. On rowsw, 3 rounds
  (logs/e33_process_rowsw_typed.txt), 13.25, 5.19 and 5.18 s, the claims' decode 0.194,
  0.087 and 0.011 s. Every table is equal to #237's on both. polars-genson-py's tests pass
  (262, 3 skipped)
- Left in the claims step on rel7k: inference, 0.40 s of its 0.72 s
- Found on the way: inference panics, caught and reported as invalid input, on a string
  that is a bare scalar (`12`): `trim_to_object` trims it to nothing and
  `is_json_object_array` indexes the empty slice. Two builds of different worktrees in one
  `CARGO_TARGET_DIR` share genson-core's metadata hash, and an example built after the
  other worktree's linked the stale library

### Inference's row phase, stage by stage

- `bench_rows_phase.rs` on rel7k, one thread, the second of two runs: serde validation
  0.014 s, the `wrap_root` copy 0.014 s, simd-json's tape 0.024 s, the rows' builders 0.223 s
  (tape included), their hashes 0.093 s, merging the 6,392 distinct ones (chunks of 100)
  0.359 s, the final `to_schema` 0.124 s (a 3.45 MB schema). The tape and validation are a
  small part; building, hashing and merging the rows' trees are most of it
- Each of those is linear in the size of all the rows' trees, and the trees repeat
  (`bench_repeats.rs`, each node's schema at each depth): rel7k's 7,128 rows hold 680,582
  schema nodes, 12,275 of them distinct (55x); rowsw's 2,185 rows (label maps in place)
  3,995,858 nodes, 9,401 distinct (425x). The rows themselves mostly differ (6,392 and
  2,164 distinct), so deduplicating whole rows, as inference does, saves little. A
  builder that interned its nodes would build, hash and merge each distinct subtree once

### Inference's rows as interned states

- `RowStates` (`genson_rs/interned.rs`) builds each row's schema from the token tape the
  typed decoder reads (moved to `tape.rs`) as an interned state: each distinct subtree of
  a block of rows is stored once, a list's items and a repeated key are a memoised join
  of states (`add_object` of a value on a node is the node joined with the value's own
  state), and each state's schema hash, which counts and drops repeated rows as before, is
  computed once from its parts' hashes. The distinct rows are merged into the builder in
  order and chunked as `max_builders` chunks them, by `absorb` read from the states
  (`SchemaNode::absorb_interned`, with `absorb`'s round trip of new properties, schema
  order of strategies and keyword placeholders), and the `$schema` handling of
  `add_builders`. Nothing per row is allocated but its new states; serde's validation,
  the `wrap_root` copy, simd-json's tape, the per-row `SchemaNode` trees, their hashing
  and their freeing are gone
- A node skips a state it has absorbed: absorbing one again, after anything else, leaves
  it as it is. That skips less than the 55x repetition promised: at first 39,582 of
  268,581 absorbs on rel7k, since a subtree repeats mostly under other paths (the same
  snak under P31 and P279), and the main tree gains 73,521 properties (claims' 2,269
  property keys, each with its statements' subtree). Known by the schema hash (`absorb`
  reads no more of a node than the hash covers) rather than by block and index, the
  states skip across blocks and the absorbs fall to 142,062; comparing the few keyword
  names a strategy holds in turn rather than by SipHash takes the rest of the merge to
  41-84 ms
- Where the rows might be read differently, nothing changes and the usual path runs,
  which also raises its errors: a row of several lines (with the `\n` delimiter), one that
  is not an object (without `wrap_root`), text serde_json rejects as a `Value`, numbers
  simd-json rejects, a brace inside a string without a delimiter, a field forced to be a
  map, and a list of 10 items or more whose items hold a node with both an object and a
  list
- Found on the way, all in the usual path and left as they are:
  - simd-json (with `big-int-as-float`) rejects text serde accepts, so inference fails
    ("invalid JSON input") on a row with a negative integer below `i64::MIN`
    (`-9223372036854775809`), a float past `f64::MAX` (`1e999`, `1.7976931348623159e308`),
    or a lone low surrogate (`"\udc00"`); it accepts a lone high one. serde's
    `IgnoredAny` validation accepts all of these; `serde_json::Value` rejects the floats
    and both surrogates but reads the integer as a float (`probe_simd.rs`)
  - Without a delimiter, a row is split after every `}` that closes its braces, counting
    braces inside strings: `{"t":"}"}` is split in two and fails
  - `ListStrategy::add_object` merges a list of 10 items or more in parallel pieces with
    `absorb`, which puts a piece's bare objects after its lists, so where a node of the
    items holds both a bare object and a list the schema's `anyOf` order depends on the
    number of threads: one row (`f0` a list of 13 items, among them `{}`, a list and a
    record) infers the record first with 1-3 threads and the list first with 4 or 8, 20
    of 20 runs each. The interned path declines such rows, so its output is the usual
    path's on every machine; a fix (merging the pieces as `add_object` does) would make
    inference independent of the core count, for the maintainer to decide on
  - `preprocess_force_field_types` changes a schema only for a field forced to be a map,
    but walks the whole schema whatever is forced: with wikidata-pq's options (`mainsnak`
    forced to a record) it reads all 3.45 MB of rel7k's merged schema and changes nothing
- Instructions, one thread, read and inference (`bench_infer_wpq.rs`, `bench_infer.rs`):
  rel7k 4.04G at #245 and 2.22G interned (the row phase 0.38G, the merge 0.24G; the rest is
  rewriting the merged schema: `to_schema`, map unification 0.63G, freeing its `Value`s
  0.21G); event logs (20,000 rows) 1.84G and 0.61G
- Time, read and inference, with the schema hash and processed count equal in every run:
  rel7k on one thread 0.81-1.10 s and 0.30-0.40 s, on four 0.36-0.40 s (CPU 1.05-1.18 s)
  and 0.21-0.26 s once warm (CPU 0.45-0.54 s; the merge, 44-105 ms, varies most); rowsw with its label maps extracted 0.19-0.20 s and
  0.06-0.09 s, rows200 0.14-0.16 s and 0.06-0.08 s; and, the default options (Avro): event
  logs (100,000 rows) 0.37-0.56 s (CPU 1.4-2.1 s) and 0.056-0.062 s (CPU 0.18-0.19 s);
  narrow200k 0.27-0.38 s and 0.031-0.041 s; label maps 0.18-0.26 s and 0.09-0.17 s, the
  rest of their cost in map inference after the merge
- Tests: the row phase against the rows' own builders (`process_json_strings_parallel`)
  on 1,200 random sets of rows (objects over a few keys, escaped and repeated keys, empty
  objects, lists short and long, of one kind of item or mixed, every number form, escapes,
  whitespace; repeated rows; in some sets invalid rows, numbers simd-json rejects, braces
  in strings and documents over two lines) under both delimiters, with and without
  `wrap_root`, three `$schema` settings and four chunk sizes: the builders' schemas, key
  order included, and processed counts, or a decline where the builders fail or might
  differ; and rows that differ only in `required`. genson-core's other tests (inference
  now takes the interned path in them) and genson-cli's pass. Of 20 mutants
  (logs/e37_interned_mutants.txt) 15 fail the tests and 5 are equivalent
- `process()` on rel7k, 6 interleaved rounds (logs/e35_process_rel7k_interned.txt), median
  wall, CPU, peak RSS: master 2.679 s, 4.91 s, 449 MB; #244 2.129 s, 3.52 s, 434 MB; #245
  2.042 s, 3.05 s, 376 MB; interned 1.764 s, 2.23 s, 364 MB. The claims step 1.405, 0.867,
  0.768 and 0.569 s wall, 3.38, 2.02, 1.60 and 0.90 s CPU; its inference 0.730, 0.403,
  0.412 and 0.234 s. On rowsw (logs/e36_process_rowsw_interned.txt) #244 5.60 s, #245
  5.14 s, interned 4.77 s, inference 0.144, 0.146 and 0.050 s. Every table is equal to
  #237's on both

### The merged schema's rewrite, in place

- After the row phase, inference rewrites the merged schema as `Value`s. On rel7k at #246
  (four threads, a patch timing each step, not kept): `to_schema` 23-51 ms,
  `preprocess_force_field_types` 9-16 ms, `rewrite_objects` (map inference and
  unification) 100-166 ms, `reorder_unions` 0.02 ms, the Avro conversion 5-6 ms. The
  merged schema is 3.45 MB of JSON and the rewritten one 4,224 bytes: the statement
  schemas of claims' 2,269 property keys unify into one map value
- The rewrite on its own (`profile_rewrite_merged.rs`: the merged schema read from a file,
  then the three passes; one thread) is 1.12G instructions, 0.23G of them reading the
  file. Copying the first schema in `unify_field_schemas_sequential` is 0.30G (27%), and
  freeing the copies and what was built from them most of the rest: each step of a fold
  called `check_unifiable_schemas` on the schema so far and the next, which builds a new
  schema from copies of both, including the fields of the schema so far that the next
  lacks or holds the same
- The schema so far is now owned by the fold, and a step moves its parts into the result
  (`unify_pair`, `unify_record_pair`, `unify_array_pair`, `unify_item_pair`,
  `schemas_compatible_owned`): a field of only the schema so far is moved and made
  nullable, a field equal in both is kept, and only what the next adds is copied. The
  first schema is borrowed until a step changes it; the parallel fold's halves, owned
  already, merge the same way. Where a pair could come out otherwise the general
  functions run: records with `anyOf` fields (unified apart and placed last), a scalar
  item and a record item (promotion), a map with an empty record, and debugging (so that
  it reports as it did). `preprocess_force_field_types` runs only when a field is forced
  to be a map, and the Avro conversion takes the schema rather than a copy
- The rewrite on its own: 0.68G instructions (0.23G reading the file, 0.05G the forced-type
  pass the harness still runs); on four threads 76-183 ms (median 103) and 56-95 ms
  (median 63), 10 runs each, with the same hash of the rewritten schema
- Read and inference on one thread: rel7k 2.13G and 1.66G instructions; labels 1.49G both
- Read and inference on four threads, 2 interleaved rounds of 5 runs, the first of each
  dropped (logs/e39_infer_owned_unification.txt), median wall and CPU, #246 and now:
  rel7k 0.214 and 0.166 s, CPU 0.425 and 0.334 s; rowsw with its label maps extracted
  0.044 and 0.038 s; event logs, narrow200k, labels and rows200 within the run-to-run
  spread (0.067/0.065, 0.038/0.041, 0.114/0.113, 2.40/2.32 s). Schema hashes and processed
  counts are equal in every run
- `process()` on rel7k, 6 interleaved rounds of #246 and this section's three steps
  (logs/e43_process_rel7k_gold4.txt), median wall, CPU, peak RSS: #246 1.912 s, 2.37 s,
  356 MB; this 1.744 s, 2.18 s, 358 MB. The claims step 0.727 and 0.526 s, its inference
  0.315 and 0.204 s. On rowsw (3 rounds, logs/e44_process_rowsw_gold4.txt) within the
  run-to-run spread: 5.22 and 5.15 s, inference 0.062 and 0.057 s. Every table is equal
  to #246's on both
- Tests: `test_owned_unification_matches_the_borrowed` unifies 3,000 random pairs (records
  over shared and many keys, nullable either way round, required fields, arrays with and
  without items, maps, empty records, legacy nullable wrappers, `anyOf` unions, schemas
  without a type, each pair a schema and one derived from it) the owned way and the
  borrowed way, and folds runs of 3-23 of them in order and by halves both ways: the
  same schemas, key order included, or both fail; 2,101 pairs and 1,538 runs unify. Of
  13 mutants (logs/e38_owned_unification_mutants.txt) 11 fail it and 2 are equivalent,
  one of them taken (an array in a legacy wrapper is unwrapped and its items moved, as
  the borrowed way reads them)
- Found on the way, left as it is: unification is not idempotent in field order. A record
  unified with one of the records it came from puts the fields that record holds first:
  folding `{a, x}`, `{a, y}` gives `a, x, y`, and folding `{a, x}`, `{a, y}`, `{a, y}`
  gives `a, y, x`. So a map value's field order depends on how often each key's schema
  repeats and on whether the fold runs in order or (from 50 schemas) by halves, and
  repeated schemas cannot be skipped in a fold without changing it

### Parallel column encoding

- genson's Parquet output went through `ArrowWriter::write` batch by batch, which encodes
  and compresses a row group's columns one after another on the calling thread. rel7k's
  normalised claims (`bench_write.rs`, the typed batches written as `write_struct_column`
  writes them) are 3,038,418 bytes in one row group of many leaf columns: 0.107-0.193 s per write
- `write_batches` writes the same file from the batches: rows go to row groups as there
  (`max_row_group_row_count` each; genson's writer sets no byte limit or content-defined
  chunking), the batches' leaf columns are computed in parallel, each column's writer
  (from `into_serialized_writer`) encodes its leaf of each batch in turn on the rayon
  pool, and the column chunks are appended in order
- The same bytes, 0.037-0.060 s per write (3 interleaved rounds of 8 writes,
  logs/e41_write_batches.txt)
- `process()` (logs/e43, e44, as above): rel7k 1.581 s wall, 2.11 s CPU, the claims step
  0.426 s, genson's write within it 0.134 and 0.042 s; on rowsw genson's write 0.063 and
  0.024 s, the rest within the run-to-run spread. Every table is equal to #246's on both
- Tests: `test_write_batches_matches_arrow_writer` writes an `id` column and a struct
  column (a string, a list of integers and a struct of a float, any of them null) in
  batches of uneven sizes, some empty, with `ArrowWriter` and with `write_batches`, under
  four layouts (genson's, small pages and row groups of 37, row groups of 1 with page
  statistics and bloom filters, no dictionary or offset index and row groups of 50) and
  0 to 300 rows: the files are equal byte for byte. Each of 4 mutants (no row-group split,
  chunks reversed, only the first batch written, the last row group dropped) fails it

### Rows with braces in strings, and the Avro conversion

- Without a delimiter, the usual path splits a row after each `}` that brings its count of
  braces, those inside strings too, back to zero, and reads each piece apart. The interned
  row phase declined any row with a brace in a string, and a declined row sends the whole
  input the usual way: 17 of rows200's 200 entities hold one (LaTeX in descriptions,
  `{\displaystyle ...}`), all of them balanced so that the usual path reads them whole, and
  inference on rows200 built every row's tree (2.9 s for the rows, a 0.32 s serial merge,
  1.57 GB peak RSS on the first run). rows20 has 2 such rows; events, narrow200k and labels
  none
- A row now declines only where the usual path would split it: where a `}` before its end
  (or, with `wrap_root`, anywhere in it, the wrapping object's own `}` coming after) brings
  the count to zero. A row it reads whole is read whole here too
- Read and inference, 2 rounds of 4 runs each in a fresh process
  (logs/e42_braces_infer.txt), the previous commit and this one: rows200 2.43-5.86 s (CPU
  5.6-17.7 s, peak RSS 1.71 GB) and 1.31-1.81 s (CPU 1.95-2.44 s, 508-522 MB); rows20
  0.93-1.69 s (327-334 MB) and 0.80-1.26 s (238-246 MB); labels unchanged; the same schema
  hashes and processed counts. Most of what is left of rows200's time is the Avro
  conversion (below)
- wikidata-pq's claims inference reads with the `\n` delimiter, which the rule does not
  touch: `process()` (logs/e43, e44) rel7k 1.625 s, rowsw 5.15 s, within the run-to-run
  spread of the previous step; every table equal to #246's on both
- Tests: `test_interned_rows_match_the_builders` now puts braces in strings in a third of
  its sets (balanced, opening, and in a third of those closing early, which the usual path
  then fails to read): of 1,200 sets 694 are compared with the builders (82 of them with
  braces in strings and no delimiter) and the rest decline. All 5 mutants of the rule fail
  the tests (logs/e40_braces_mutants.txt)
- Found while measuring, outside genson: on labels (1,000 rows of label maps, default
  options), `to_avro_schema` is 1.06G of inference's 1.49G instructions, nearly all of it
  compiling regular expressions. avrotize's `avro_name` and `avro_namespace` (and its case
  conversions) call `Regex::new` on every call, and a wide record schema (labels' 473
  language keys) names a record and a field per key. With the 11 patterns compiled once
  (`avrotize_lazy_regex.patch`, statics behind `LazyLock`; measured only, not part of this
  repository), the schemas are the same and inference takes, warm: labels 0.113-0.130 s
  and 0.018-0.026 s, rows200 (with the rule above) 1.48-1.57 s and 0.45-0.55 s; rel7k,
  whose rewritten schema is small, is unchanged

### The row phase's lookups

- At the stack's top so far (`bench_infer_1t.rs` and `bench_infer_wpq_1t.rs`: inference in a
  one-thread pool, so that every stage runs on the thread callgrind follows, counted from
  `infer_json_schema_from_strings`), inference is 1.45G instructions on rel7k (the row
  phase 0.64G: lexing 0.23G, interning 0.32G; the rewrite 0.40G, `to_schema` 0.21G, Avro
  0.05G), 1.69G on narrow200k (the row phase 1.62G, 96%: lexing 0.55G, interning 0.81G)
  and 0.61G on event logs (20,000 rows; the row phase 92%). narrow200k has 8 distinct
  schemas in 200,000 rows and event logs 456 in 20,000, so nearly every row's interning
  finds what it builds already there
- Interning a row's object built its `Obj` (two boxed slices) and the boxed strategies of
  its state to look them up, and freed them when they were there; a list built its boxed
  strategies the same way. A row's objects (every property required) are now found by
  their properties as a slice, lists by their items' state, and states by a borrowed
  slice, with nothing allocated unless the state is new
- Rows of the same shape repeat their keys and objects at the same positions: a row's
  keys and objects are first compared with the previous row's at their position (the
  key's text, the object's properties) and found by hash only if they differ
- Without a delimiter, the scan for braces that close the row early (`read_whole`) reads
  only the braces, found with `memchr2`
- Instructions, inference on one thread, the previous step and now: narrow200k 1.69G and
  1.20G (-29%), event logs 0.61G and 0.45G (-27%), rel7k 1.45G and 1.39G (-4%), labels
  1.49G and 1.39G (the rest is avrotize); the same schema hashes and processed counts. On
  rel7k the lookups alone gave 1.37G: the position caches cost its rows, whose shapes
  rarely repeat, 0.015G (a variant that stops looking when the previous row's lookups
  mostly missed costs more: the bookkeeping changes how `value` is inlined)
- Read and inference on four threads, both built alike, 4 interleaved rounds of 6 runs, the
  first of each dropped (logs/e46_infer_rowstates.txt), median wall and CPU: event logs
  0.067 and 0.058 s, CPU 0.226 and 0.184 s; narrow200k 0.035 and 0.032 s, CPU 0.116 and
  0.097 s; labels and rel7k within the run-to-run spread (0.120/0.117 and 0.196/0.206 s)
- Tests: the interned row tests as before (the states are the same, so the builders and
  schemas compared are); of 7 mutants (logs/e45_rowstates_mutants.txt) 5 fail them and 2
  are equivalent (a stale or misaligned position falls back to the lookup by hash)

### The interned merge, each property on its own thread

- The interned merge absorbed each distinct row into the builder in turn on one thread,
  every row's subtree through every node it reaches: on rel7k 6,392 rows into a claims
  object of 2,269 property keys, 36-136 ms (median about 50) of inference's 0.17-0.24 s
  on four threads, the longest serial stage left. The usual path already merges its
  chunks' builders with each property's nodes on their own threads (`absorb_all`)
- `absorb_interned_all` does the same for the states: a node takes its states in their
  order, doing all of its own part of `absorb_interned` (the states it skips, its
  strategies, their order and the typeless one merged into a new one, numbers widened,
  an object's keyword placeholders, `required` and its new properties' nodes, added
  empty where they were added before), with each child's states held back
  (`absorb_interned_deferred`); then each child takes its own states in their order,
  an object's children on their own threads (`absorb_deferred`). A node reads nothing of
  its children and they nothing of each other, and each node is given the same states in
  the same order as before, so the tree is the same. The builder takes the root's rows
  chunk by chunk, each chunk's `$schema` after its rows as before, and the children's
  states after the last chunk (`add_interned_row_chunks`)
- The same schema hashes and processed counts on every corpus; instructions (one thread)
  within 1% (rel7k 1.39G and 1.41G: the groups)
- Inference on four threads, 4 interleaved rounds of 6 runs, the first of each dropped
  (logs/e47_infer_parallel_merge.txt), median wall and CPU, the previous step and now:
  rel7k 0.236 and 0.167 s (fastest runs 0.158 and 0.124 s), CPU 0.432 and 0.384 s; the
  merge's own time (genson's profile, 10 runs each) 36-136 ms and 10-35 ms; rowsw with
  its label maps extracted, event logs and labels within the run-to-run spread (their
  merges are a few milliseconds)
- `process()`, 6 interleaved rounds of #249, #250 and this on rel7k, 3 on rowsw
  (logs/e48_process_rel7k_merge.txt, e49_process_rowsw_merge.txt), median wall 1.711,
  1.763 and 1.558 s; the claims step 0.500, 0.586 and 0.412 s, its inference 0.189, 0.331
  and 0.139 s (this series spread widely: 0.164-0.645, 0.151-0.529 and 0.097-0.173 s).
  rowsw within the spread. Every table equal to #249's on both
- Tests: `test_interned_merge_keeps_each_chunks_keywords_in_place` (rows that are empty
  objects, then rows with fields, nested objects and lists, under four chunk sizes, three
  `$schema` settings and both delimiters: the interned merge gives the rows' builders'
  schema, compared as text, so keyword order counts) and the interned row tests as
  before; of 6 mutants
  (logs/e50_merge_mutants.txt) 3 fail them and 3 are equivalent, one of them taken
  (a strategy that replaces the typeless one takes its slot)

### The merged schema as a DAG

- After the merge, inference wrote the builder's schema out as `Value`s and rewrote it in
  place. On rel7k that schema is 3.45 MB of JSON, 261,798 values, and only 5,586 of them
  are distinct: its 2,269 claims property keys hold 755 distinct schemas, and those share
  most of their parts. Writing it out was 205M instructions (one thread), rewriting it
  398M, of inference's 1.41G; the rewritten schema is 4 KB
- The builder now writes its schema as a hash-consed DAG (`dag.rs`, `SchemaBuilder::to_dag`,
  each strategy's `to_dag` beside its `to_schema`): each distinct value is stored once and
  named by an id, keys and strings by symbols. The rewrite runs on the DAG
  (`map_inference/on_dag.rs`, `on_dag/unification.rs`), function for function and case for
  case the one of `map_inference.rs` and `unification.rs`, with ids for `Value`s, and only
  the rewritten schema becomes a `Value`:
  - `Value` equality, which ignores the order of an object's keys, is `Dag::same`: the
    two ids, or the ids of their canonical forms (keys sorted), computed once per value
  - a schema is rewritten once per field and position it is met in, and a pair unified
    once per path, remembered: each is a function of these and the config
  - pairs unify as `check_unifiable_schemas` unifies them; the owned ways of
    `unification.rs` give the same schemas (`test_owned_unification_matches_the_borrowed`),
    and with ids nothing is moved. What runs on the rayon pool there runs in turn here,
    by the same halves, to the same result
  - debugging keeps the `Value` way, so that it reports as it did
- The same schema hashes and processed counts on every corpus. Instructions, one thread
  (logs/e52_dag_instructions.txt): rel7k 1.41G and 1.05G, writing and rewriting the
  schema 603M and 233M (writing the DAG 135M, rewriting it 96M); labels 1.392G and
  1.396G (its Avro conversion is 1.06G of both)
- Inference, four threads, 2 interleaved rounds of 5 runs (logs/e51_infer_dag.txt),
  median wall and CPU, #251 and now: rel7k 0.136 and 0.080 s, CPU 0.340 and 0.170 s;
  rowsw with its label maps extracted 0.042 and 0.024 s, CPU 0.093 and 0.053 s; event
  logs, narrow200k, labels and rows200 within the run-to-run spread
- The rewrite on the DAG runs on one thread, and on rel7k on four it is now the longest
  stage: writing the DAG is mostly hashing property names into symbols
- Tests: `test_to_dag_matches_to_schema` (builders of random rows and random schemas,
  four `$schema` settings), `test_rewrite_on_dag_matches_rewrite_objects` (3,000 random
  schemas, adversarial ones among them: legacy nullable wrappers, `anyOf`s, maps with
  properties, tuples, empty records and items, fields named as keywords, map candidates of
  one family of schemas; under random configs of thresholds, unification, scalar
  promotion and forced types; the `Value` way on one thread and on four) and
  `test_inference_on_dag_matches_values` (240 sets of claims rows and other JSON under
  wikidata-pq's config, the defaults and random configs, three chunk sizes, 1, 2 and 4
  threads), each comparing schemas as text, so key order counts

### Writing the DAG without allocating per node

- Writing the DAG was 135M instructions of rel7k's 233M for the DAG stage: for each of
  the builder's 82,617 nodes a few vectors and objects being built, and the type names
  and `required` names hashed into symbols and interned again (291,367 interns and
  229,162 symbol lookups for 5,589 distinct values; logs/e53_dag_build_counts.txt)
- Now `{"type": t}` and each symbol's string are made once per DAG and taken by id, a
  node of one strategy is its strategy's schema, and the entries and items of the objects
  and arrays being built go on two stacks the DAG keeps (`object_from`, `array_from`);
  a strategy with extra keywords takes the way it took
- Against #252 (the same release profile, logs/e55_dag_build_instructions.txt): writing
  the DAG rel7k 134.6M and 67.8M instructions, rowsw 21.3M and 11.7M, rows200 390.0M and
  233.7M; the DAG stage 232.9M and 164.7M, 42.7M and 32.9M, 439.7M and 283.3M; inference
  1.050G and 0.982G (-6.5%), 0.400G and 0.391G, 16.93G and 16.76G; the rest unchanged.
  Allocations writing the DAG on rel7k 283,959 and 31,116 (8.5 and 2.2 MB requested),
  in inference 1,409,831 and 1,156,988: what writing the DAG no longer allocates, and no
  more. Interns 291,367 and 82,922; symbol lookups 229,162 and 170,429, now mostly the
  properties' names and `required` (25.2M of the 67.8M). Peak RSS unchanged (the input
  is most of it)
- Inference, four threads, 2 interleaved rounds of 5 runs (logs/e54_infer_dag_build.txt):
  rel7k 0.076 and 0.066 s, CPU 0.165 and 0.154 s; the rest within the spread
- What is left is not in writing the DAG: halving it gained 6.5% of rel7k's inference

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
  slim200 from it), labels and narrow200k, and the chunk-shaped files above, made from
  them and from 60 fetched entities
- Disk throughput on the machines that run wikidata-pq, which decides what the
  uncompressed output costs in wall time
