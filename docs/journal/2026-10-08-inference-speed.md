# 2026-10-08 Inference speed and map unification

Work on the speed and the structure of schema inference in genson-core, the map
unification included. Every change here keeps the inferred schema byte-identical unless
a section says otherwise.

## How inference runs

`infer_json_schema_from_strings` (genson-core/src/schema.rs) runs in two phases:

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
2. Unification: one function classifies a schema (array, map, record, scalar, …), in
   place of six `is_*_schema` predicates that each re-parse the nullable forms and
   allocate a `"null"` string per comparison; borrow field maps instead of cloning
   them.
3. `rewrite_objects`: remove the duplicated force-promotion block and the clones of
   value schemas that are about to be moved.
4. Merge `SchemaNode`s directly instead of going through `to_schema` and `add_schema`,
   for the list items and builders that are reduced in parallel, and for the row
   builders.

## Progress

- Stage 1: `wrap_documents` writes `{"<field>":` and `}` around each document's bytes
  (genson-core/src/schema.rs), and `SchemaNode::create_strategy_for_kind` reserves
  exactly one strategy for a node's first (genson-core/src/genson_rs/node.rs) — a
  `BasicSchemaStrategy` is 248 bytes, and a Vec's first push reserves four.

| corpus, options       | baseline wall / CPU | `wrap_documents` | + one strategy |
|-----------------------|---------------------|------------------|----------------|
| rows200, `WIKIDATA=1` | 4.92s / 12.51s      | 4.06s / 10.01s   | 2.79s / 8.32s  |
| rows200, default      | 3.20s / 8.98s       | —                | 1.69s / 5.81s  |
| labels, default       | 0.21s / 0.68s       | —                | 0.18s / 0.62s  |

## Current State

- `wrap_root` builds each wrapped document from the input's own bytes, so duplicate keys
  and number formatting reach simd-json as written, the same as without `wrap_root`
  (genson-core/src/schema.rs `wrap_documents`)
- `bench_claims` prints wall time, CPU time and a hash of the schema, and takes the
  claims options with `WIKIDATA=1` (genson-core/examples/bench_claims.rs)
- genson-cli prints two different schemas for
  `genson-cli/tests/data/claims_fixture_x30.jsonl` with default options at da646ea,
  before any change here — 2 of 8 runs give a record a `required: ["P813", "P854"]` that
  the other 6 runs lack
