# 2026-10-15 Unification algebra

Chats with Claude Fable on formalising the merge operation behind map unification, and
on which algebraic properties it holds, are stored in `2026-10-15-unification-algebra/`
next to this entry. This entry records the operation as implemented at 96ef9d5.

## Current State

- `check_unifiable_schemas` merges a list of schemas by dispatching on their kind — all
  arrays, all maps or empty records, all records or empty records, all scalars — and
  returns `None` for any mix of kinds
  (genson-core/src/schema/map_inference/unification.rs:678-797)
- `check_unifiable_schemas` returns `{"type": "object", "additionalProperties":
  {"type": "string"}}` when every schema is an empty record, so an empty map's value type
  comes from no data (genson-core/src/schema/map_inference/unification.rs:725-734)
- `unify_field_schemas` folds one field's schemas left to right with `unify_field_pair`
  when there are fewer than 50 of them or `wrap_scalars` mixes scalars with records, and
  otherwise splits them in halves on rayon
  (genson-core/src/schema/map_inference/unification.rs:453-469)
- `unify_field_pair` tries `schemas_compatible` and scalar promotion before
  `check_unifiable_schemas`, while the halves of `unify_field_schemas_parallel` are
  joined by `check_unifiable_schemas` alone — the left fold and the halves combine
  schemas with different functions
  (genson-core/src/schema/map_inference/unification.rs:402-431, 434-450)
- `check_unifiable_item_schemas` falls back to `unify_anyof_schemas` for arrays whose
  items mix scalars and records when `wrap_scalars` is set; record fields get no such
  fallback (genson-core/src/schema/map_inference/unification.rs:802-817)
- `unified_values` decides a candidate map's value schema: the shared schema when every
  value is equal once nulls are set aside, `None` without `unify_maps` or when a key is
  in `no_unify`, and `check_unifiable_schemas` (or `check_unifiable_item_schemas` for
  arrays) otherwise (genson-core/src/schema/map_inference.rs:451-512)
- Tree merging during the build phase goes through `SchemaNode::absorb` and
  `absorb_all`, recorded in docs/journal/2026-10-08-inference-speed.md

- Reversing the rows of `claims_fixture_x4.jsonl` and `claims_fixture_x30.jsonl`
  (genson-cli/tests/data) changes which keys are `required`, not only the order of
  properties, under `--ndjson --map-threshold 0 --unify-maps --wrap-root claims` (the
  options of genson-cli/tests/claims_fixtures.rs) — with the rows reversed, the
  `datavalue` record under `references` in x4 gains `"required": ["id", "labels"]`,
  although some of its rows are promoted scalars (`datavalue__string`) holding neither
  key
- With `--no-wrap-scalars` added, x4 gives the same schema in both row orders apart from
  property order, and x30 still does not — the `required` of each qualifier's
  `datavalue` record follows the first row's kind of value (`["id", "labels"]` or
  `["time", "timezone", ...]`) in each order
- `unify_record_schemas` counts the records holding each field and marks a field
  required only when every record holds it, whatever their order
  (genson-core/src/schema/map_inference/unification.rs:533-540) — the row-order
  dependence of `required` above arises outside that function
- genson-cli passes a file read without `--pq-column` to genson-core as one string, so
  inference takes the sequential path, where `--max-builders` has no effect and no row is
  deduplicated (genson-cli/src/main.rs:238-246, `PARALLEL_THRESHOLD` of 10 strings in
  genson-core/src/schema.rs:22); `--pq-column` passes one string per Parquet row, as
  polars-genson-py does for a column — the row-order results above are from files, on
  the sequential path
- On the parallel path, 16 claims rows in a Parquet column (the `MIXED_SOURCES` of
  genson-cli/tests/wikidata_config.rs, with the 4 rows of `claims_fixture_x4.jsonl` twice)
  give a different schema and Avro schema in reverse order, beyond property order, under
  wikidata-pq's claims options
- On the same 16 rows, `--max-builders` 1, 3, 7 and 100, and 1, 4 and the default number
  of rayon threads, give byte-identical schemas and Avro schemas under wikidata-pq's claims
  options; `--max-builders 3` splits them into 6 chunks and keeps 8 distinct row schemas
- `SchemaNode::add_schemas` into a node already holding a schema gives a different schema
  from `add_schema` of each schema in turn — first seen at 96ef9d5 with documents
  `{"c": 46, "b": null, "a": 34}`, `"s"`, `{"b": {}}`, where the batch gives `b` as
  `integer` and the sequence as `["integer", "null", "object"]`; into a new node the two
  agree on 3000 random batches (`test_add_schemas_matches_adding_in_turn` on branch
  `perf/schema-merge-no-clones`)
- `ObjectStrategy::add_schemas` merges more than 3 properties on rayon and puts each
  merged node in with `properties.insert`, which replaces a node the strategy already
  held for that property (genson-core/src/genson_rs/strategy/object.rs, Phase 2 of
  `add_schemas`)
- `just bench` runs the Python benchmarks in bench/ (Justfile `bench`), and
  genson-core/benches/schema_inference.rs calls `build_json_schema` directly — neither
  reaches the row deduplication in `process_json_strings_parallel`; examples/bench_claims.rs
  does, passing one string per row

## Missing

- No test checks that unification gives the same schema whatever the order or grouping
  of its inputs, and no crate in the workspace depends on proptest or quickcheck
- No document in docs/concepts states the merge as an operation with an identity or
  algebraic laws; docs/concepts/inference.md and maps-and-records.md give its rules as
  prose

## Divergence

- docs/changelog.md (Unreleased) states that which keys are required no longer depends
  on the order of the rows; reversing the rows of `claims_fixture_x4.jsonl`,
  `claims_fixture_x30.jsonl` and the 16 Parquet rows above still changes `required`
- docs/changelog.md lists the changes of #220 under Unreleased, and polars-genson 1.0.1
  (`5a1be13`) was released after #220

## Progress

Work packages are those of `2026-10-15-unification-algebra/meta/brief.md`, one PR each
except WP2 and WP3, which share one.

- Branch `docs/unification-algebra`: this entry, the Fable chats and their outline, and
  the brief
- Branch `test/wikidata-config-snapshots` (WP0): `9fa4e04` re-locks Cargo.lock, which
  listed genson-core and genson-cli as 0.9.5 and polars-jsonschema-bridge as 0.9.1 after
  their manifests moved to 1.0.0; `e23ab9e` adds genson-cli/tests/wikidata_config.rs
  with 20 snapshots — 9 claims fixtures read as files, and 16 of their rows as a Parquet
  column — plus the `--max-builders` and thread-count checks on the Parquet rows, 22
  tests taking about 20 s, under wikidata-pq's claims options (`--ndjson
  --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record
  --force-scalar-promotion mainsnak,datavalue,precision,latitude,longitude --no-unify
  qualifiers --max-builders 100`, from `CLAIMS_INFERENCE_OPTIONS` in wikidata-pq's
  process.py) — the brief's `--wrap-scalars` flag does not exist in genson-cli, which
  wraps scalars unless given `--no-wrap-scalars` (genson-cli/src/main.rs:175)
- Branch `perf/schema-merge-no-clones` (WP2 and WP3), on top of
  `test/wikidata-config-snapshots`: `e0e031a` passes `&[&Value]` to
  `SchemaNode::add_schemas` from the object and array strategies, removing four clones
  of every sub-schema in a batch (object.rs, array.rs), with
  `test_add_schemas_matches_adding_in_turn`; `9ce9e40` compiles each `patternProperties`
  regex once per strategy, in a `PatternRegexes` list kept in the order of
  `pattern_properties`, where it was compiled for every key of every object, with
  `test_pattern_properties_take_matching_keys`, which passes before and after the change
- Branch `fix/row-dedup-hash-128` (WP4), on top of `test/wikidata-config-snapshots`:
  `4f3e50e` deduplicates rows on `SchemaBuilder::schema_hash_128` (xxh3, 128 bits),
  leaves the public `schema_hash` (xxh64) as it is, extends
  `test_hash_schema_matches_schema_equality` to the 128-bit hash, and adds a changelog
  entry; `bench_claims` with `WIKIDATA=1` on `claims_fixture_x30.jsonl` repeated 4 times
  (120 rows, 30 distinct row schemas) gives a median of 1.52 s at master and 1.50 s on
  the branch over 6 alternated runs each, with the same schema hash
- WP2 and WP3 change code that only callers of the public `genson_core::genson_rs` API
  reach: `SchemaBuilder::add_schemas` (genson-core/src/genson_rs/builder.rs:160) has no
  caller in the workspace, and a `patternProperties` keyword enters a builder only in a
  schema passed to `SchemaBuilder::add_schema` or `add_schema_mut`, which inference does
  not call (genson-core/src/schema.rs, genson-cli, polars-genson-py)
- `SchemaNode` is `pub(crate)` (genson-core/src/genson_rs/mod.rs:15), so changing
  `SchemaNode::add_schemas` to take `&[&Value]` (WP2) leaves the public API as it is
- WP4: rows are deduplicated on the u64 xxh64 `schema_hash` (genson-core/src/schema.rs
  388-466, genson-core/src/genson_rs/builder.rs:142); xxhash-rust is built with the
  `xxh64` feature only (Cargo.toml:37)
- Local branches from before this work, not reviewed here: `claude/polars-map-dtype-compat-20omwz`
  (PR #216, superseded by #217-#219 per 2026-10-07-polars-2-maps.md), `map-video`,
  `sonic-rs`, `fix/fxhash-key-order`, `fix/normalise-integer-in-float-field`,
  `reenable-ci-arches`, `dependabot/github_actions/actions/attest-build-provenance-3`,
  `worktree-agent-a7b112cd7e27d1ac7`, `worktree-agent-afb6f11725de383f4`; `polars-schema-deser`
  is merged into master
