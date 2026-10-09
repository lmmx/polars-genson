# 2026-10-08 Inference speed and map unification

Work on the speed and the structure of schema inference in genson-core, the map
unification included. Every change here keeps the inferred schema byte-identical unless
a section says otherwise.

## How inference ran at da646ea

`infer_json_schema_from_strings` (genson-core/src/schema.rs) runs in two phases (stage 2
below replaces the schema values of phase 1 with nodes):

1. **Build.** Each input string is parsed by simd-json and folded into a tree of
   `SchemaNode`s (genson-core/src/genson_rs). With 10 or more strings, each one gets its
   own builder on a rayon worker; the builder is turned into a `serde_json::Value`
   (`to_schema`), hashed, and the distinct row schemas are merged into one builder per
   chunk of `max_builders` rows (`add_schemas_mut`). One builder becomes one JSON Schema.
2. **Rewrite.** On that one schema: `preprocess_force_field_types`, then
   `rewrite_objects` (genson-core/src/schema/map_inference.rs) decides which objects
   are maps, using `check_unifiable_schemas` (map_inference/unification.rs) to merge
   the value schemas of a candidate map, then `reorder_unions`.

## Benchmarks

- `bench/claims_rows.py 200 rows200.ndjson` writes 200 rows (216.8 MB) of Wikidata
  claims, each a random half of the properties of one of 11 entities from the
  genson-cli fixtures and `bench/data/claims.parquet` (seed 0).
- `genson-core/examples/bench_claims.rs` times inference on an NDJSON file, one string
  per row, `max_builders=100`. `WIKIDATA=1` adds `map_threshold=0`, `unify_maps`,
  `wrap_root="claims"` (the wikidata-pq claims options).
- Timings are medians over 6 runs, one process per run, binaries alternated, on a
  4-core Xeon @ 2.80GHz VM. Wall time varies by up to 40% between runs here; CPU time
  (user + system) varies less.
- A check script runs genson-cli on every fixture in genson-cli/tests/data under four
  option sets (default; claims; claims with `--avro`; forced types) and bench_claims on
  three NDJSON files — 221 outputs, compared byte for byte before and after each change.

## Baseline (da646ea)

| corpus, options              | wall  | CPU    |
|------------------------------|-------|--------|
| rows200, `WIKIDATA=1`        | 4.92s | 12.51s |
| rows200, default             | 3.20s | 8.98s  |
| `bench/data/labels.parquet` (996 rows), default | 0.21s | 0.68s |

A samply profile of rows200 with `WIKIDATA=1` (perf_event_paranoid set to 1) puts 38%
of samples in mimalloc's `mi_page_free_list_extend`, the first touch of fresh pages,
reached mostly from growing Vecs: `SchemaNode::to_schema` (15%),
`create_strategy_for_kind` (7%) and `prepare_json_bytes` (6%). By caller, the per-row
`to_schema` is 23% of samples, `prepare_json_bytes` 22% (it parsed and re-serialised
each row for `wrap_root`), building nodes 20%, `check_unifiable_schemas` 8%,
`rewrite_objects` 6%, and `hash_schema` 3%.

## Plan

1. Cheap changes on the build path: wrap the row's bytes for `wrap_root` instead of
   re-serialising it, and stop over-allocating each node's strategy list.
2. Merge `SchemaNode`s directly instead of going through `to_schema` and `add_schema`,
   for the list items and builders that are reduced in parallel, and for the row
   builders.
3. Unification: the six `is_*_schema` predicates, which each re-parse the nullable
   forms and allocate a `"null"` string per comparison, over one helper; borrow field
   maps instead of cloning them.
4. `rewrite_objects`: remove the duplicated force-promotion block and the clones of
   value schemas that are about to be moved.

## Progress

### Stage 1: the build path (b81d2c3, 30fee09)

- `wrap_documents` writes `{"<field>":` and `}` around each document's bytes
  (genson-core/src/schema.rs), where `prepare_json_bytes` parsed each document into a
  `serde_json::Value` and serialised `{field: value}` again.
- `SchemaNode::create_strategy_for_kind` reserves exactly one strategy for a node's first
  (genson-core/src/genson_rs/node.rs) — a `BasicSchemaStrategy` is 248 bytes, and a
  Vec's first push reserves four.

| corpus, options       | baseline wall / CPU | `wrap_documents` | + one strategy |
|-----------------------|---------------------|------------------|----------------|
| rows200, `WIKIDATA=1` | 4.92s / 12.51s      | 4.06s / 10.01s   | 2.79s / 8.32s  |
| rows200, default      | 3.20s / 8.98s       | —                | 1.69s / 5.81s  |
| labels, default       | 0.21s / 0.68s       | —                | 0.18s / 0.62s  |

### Stage 2: merging nodes (a89b9c9, 4ba97f6, 74f329f)

- `SchemaNode::absorb(other)` gives the node that `add_schema(&other.to_schema())` gives
  without building the schema value: a property, items or values node new to the target
  is moved in (rebuilt as `add_schema` would build it, `SchemaNode::round_tripped`), and a
  matching one is merged node by node (genson-core/src/genson_rs/node.rs,
  strategy/object.rs `ObjectStrategy::absorb`, strategy/array.rs `ListStrategy::absorb`).
- `ObjectStrategy::schema_keys` lists the keys `to_schema` writes, in order, and `absorb`
  records them as placeholder extra keywords the way `add_extra_keywords` does — the
  extra keywords fix the key order of the output, so the output stays byte-identical.
- The fold/reduce of `ListStrategy::add_object` (arrays of 10 or more items) and of
  `build_multi_json_objects_schema` (documents within one string) merge with `absorb`
  and `SchemaBuilder::add_builder`.
- Each string's builder is hashed with `SchemaBuilder::schema_hash` (`SchemaNode::hash_schema`,
  equal for equal schemas, no schema value built) and merged with
  `SchemaBuilder::add_builders`, which merges object roots in parallel across their
  properties (`SchemaNode::absorb_all`) and gives the `$schema` keyword the handling
  `add_schemas_mut` gave it (genson-core/src/genson_rs/builder.rs, genson-core/src/schema.rs).
- A field forced to `"map"` is applied to each string's nodes by
  `apply_force_field_types_to_node`, which writes out only that property, rewrites it with
  `apply_force_field_types`, and reads it back — every string takes the same path.
- genson-core/src/tests/node.rs checks `absorb` against `add_schema` of the schema (5000
  random pairs of nodes), `absorb_all` against `absorb` in turn, and `hash_schema`
  equality against equality of the schema text (20000 pairs).
- bench_claims with `URI` set to each of `AUTO`, none and `http://json-schema.org/schema#`,
  and with `FORCE` naming fields forced to `"map"`, gives the same hash before and after on
  rows200, labels and two files of mixed object, array and empty rows — both variables
  were added to a local copy of bench_claims only.

### Stage 3: `required` (01bdc92)

- `ObjectStrategy::to_schema` leaves out a `required` set that is empty, so merging two
  nodes through the schema took no account of objects that held no key in common —
  `{"x": {"a": 1}}` then `{"x": {}}` inferred `a` as required, the reverse order did not,
  and genson-cli on `claims_fixture_x30.jsonl` gave two schemas over 8 runs at da646ea.
- `absorb` intersects with every observed `required` set, empty or not
  (strategy/object.rs `absorb_keywords_and_required`); `absorb` documents this as its one
  difference from `add_schema(&other.to_schema())`.
- The genson-cli fixture schemas change only by keys dropped from `required`: 17 of the
  221 check outputs differ, all in default or forced-type options. A script comparing each
  record's `required` to the keys present in every object at its path in the data finds
  57 records that differ in `claims_fixture_x30.jsonl` at da646ea and 6 after; each of
  the 6 requires fewer keys than the data holds, under scalar promotion
  (`labels__string`), which clears `required` by design.
- `test_key_missing_from_an_empty_object_is_not_required` (genson-core/src/tests/schema.rs)
  fails at da646ea and passes after.
- rows200 with default options changes hash from `e4e9b62e4ea91b8f` to
  `b044387a11c95954`; with `WIKIDATA=1` (whose maps carry no `required`) it stays
  `ab63cbfae2d3dbf7`. All snapshot tests pass unchanged.

### Stage 4: unification (e2a5285)

- `is_scalar_schema`, `is_array_schema`, `is_object_schema`, `is_map_schema`,
  `is_empty_record_schema` and `get_scalar_type_name` are each a line over `base_type`
  (the type with nullability set aside) and `normalise_nullable`
  (genson-core/src/schema/map_inference/unification.rs).
- `schemas_compatible` compares the two schemas through `NonNullView` (moved from
  map_inference.rs) where it cloned both.
- `unify_record_schemas` groups borrowed field schemas, where it cloned every record's
  `properties` map, and moves each unified field into the result; field order and the
  order of each field's schemas are unchanged, with unified `anyOf` values still last.
- The `unify_field_schemas` family returns `Option<Value>`, and the pairwise step is
  `unify_field_pair`.

### Stage 5: `rewrite_objects` (b1ffb76)

- map_inference.rs is 600 lines where it was 906: `promote_forced_scalar`,
  `rewrite_fields`, `identical_record_values` and `unified_values` are the steps of
  `rewrite_objects`.
- `unified_values` runs only for an object that can become a map (enough keys, not a
  blocked root, few enough required keys) — it ran for every object with properties, and
  its result was dropped.
- The second copy of the nullable force-promotion branch, which could not run, is gone;
  a non-nullable promoted scalar returns at once where it fell through to a guard that
  rewrote the scalar it had just wrapped.
- genson-cli on every fixture under five further option sets (`--map-max-rk 1`,
  `--root-map`, `--no-unify`, `--force-scalar-promotion`, `--no-wrap-scalars` with
  `--force-parent-type`) gives the same output before and after: 160 comparisons.

### Timings at each commit

Medians over 6 runs, binaries alternated in each round (same caveats as above). A second
round of the first two commits gave 3.99s and 3.76s wall, so differences under about 20%
between neighbouring rows are within noise.

| commit  | rows200 `WIKIDATA=1` wall / CPU | rows200 default wall / CPU | labels wall / CPU |
|---------|---------------------------------|----------------------------|-------------------|
| da646ea | 5.11s / 13.13s                  | 3.33s / 9.31s              | 0.20s / 0.69s     |
| 30fee09 | 2.79s / 8.13s                   | 1.72s / 5.86s              | 0.20s / 0.68s     |
| a89b9c9 | 3.71s / 8.91s                   | 2.05s / 6.28s              | 0.18s / 0.64s     |
| 4ba97f6 | 2.55s / 7.60s                   | 1.48s / 5.12s              | 0.18s / 0.61s     |
| 74f329f | 3.56s / 8.45s                   | 1.56s / 5.38s              | 0.18s / 0.62s     |
| 01bdc92 | 2.41s / 7.36s                   | 2.27s / 6.53s              | 0.18s / 0.60s     |
| e2a5285 | 2.09s / 6.53s                   | 1.59s / 5.25s              | 0.19s / 0.63s     |
| b1ffb76 | 1.94s / 6.31s                   | 2.36s / 6.61s              | 0.18s / 0.64s     |

- A second run of b1ffb76 against 01bdc92 alone gave 2.46s / 6.75s against 3.22s / 8.16s
  with `WIKIDATA=1`, and 1.56s / 5.21s against 2.32s / 6.35s with default options.
- genson-cli with the claims options on `x1818_L4.jsonl` (5.5 MB, one string) takes
  0.53–0.75s at a89b9c9 against 0.67–1.88s at da646ea, five runs each.

### Measured and not kept

- A canonical key order in `ObjectStrategy::to_schema` (`type`, `properties`,
  `patternProperties`, `additionalProperties`, `required`, then the rest) fails 28
  genson-cli snapshot tests, all on key order.
- simd-json parse buffers kept per thread (`to_tape_with_buffers` with a thread-local
  `Buffers`, documents over 16 MB excepted) gave 2.48s / 6.86s against 1.96s / 6.47s with
  `WIKIDATA=1` and 2.33s / 6.51s against 1.42s / 4.96s with default options.
- `MIMALLOC_PURGE_DELAY=-1` gave a median 3.15s against 4.26s wall at da646ea with
  `WIKIDATA=1`, 8 runs each in one process; nothing in the code sets it.

## Current State

- `wrap_root` builds each wrapped document from the input's own bytes, so duplicate keys
  and number formatting reach simd-json as written, the same as without `wrap_root`
  (genson-core/src/schema.rs `wrap_documents`)
- Every merge of schema trees inside genson-core goes through `SchemaNode::absorb` or
  `absorb_all`; `SchemaBuilder::add_schema`, `add_schemas` and `add_schemas_mut` remain
  public and take schema values (genson-core/src/genson_rs/builder.rs)
- A key is required when every object seen at its position holds it, whatever the order
  of the rows and the number of threads (strategy/object.rs `absorb_keywords_and_required`)
- genson-cli on `claims_fixture_x30.jsonl` gives one schema over 8 runs with default
  threads
- The profile of rows200 with `WIKIDATA=1` at 4ba97f6 puts 37% of samples in building
  nodes from the data (20% in simd-json's `from_slice`), 24% in bench_claims reading the
  file, 12% in
  `check_unifiable_schemas`, 10% in `absorb_all`, 9% in `rewrite_objects` and 7% in
  `hash_schema`
- `bench_claims` prints wall time, CPU time and a hash of the schema, and takes the
  claims options with `WIKIDATA=1` (genson-core/examples/bench_claims.rs); rows for it
  come from bench/claims_rows.py
- docs/concepts/inference.md describes both passes of inference and the map and
  unification rules

## Missing

- No test or bench covers the parallel string path with an explicit `schema_uri` or a
  field forced to `"map"` — bench_claims takes neither as an option
- No equivalence test covers `SchemaBuilder::add_builders` directly; its `$schema`
  handling is checked by the bench hashes above only

## Divergence

- docs/concepts/key-order.md states that the same input gives the same field order on
  every run; the order of keywords within a schema object (`type` before or after
  `properties`) still depends on how rayon groups the documents of one string —
  `claims_fixture_x30.jsonl` with one thread and with four gives schemas equal as JSON
  values but with `type` placed differently in some records
- A row that is a bare scalar (`5`, `"s"`) without `wrap_root` panics in
  `is_json_object_array` (genson-core/src/genson_rs/mod.rs) on the empty slice that
  `trim_to_object` returns, and inference fails with "invalid JSON input", at da646ea
  and after
