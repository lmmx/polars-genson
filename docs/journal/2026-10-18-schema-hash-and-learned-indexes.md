# 2026-10-18 Learned indexes and the cost of `schema_hash`

Whether a learned index (a Piecewise Geometric Model, PGM) applies to genson-core, and the
per-row cost of `SchemaBuilder::schema_hash`, measured along the way. The scripts and the
patch of the rejected experiment are in `2026-10-18-schema-hash-and-learned-indexes/` next
to this entry.

## Current State

### The learned index

- `learned_index_lab` 0.1.1 (crates.io, 2026-10-15) builds a PGM index over a sorted
  `Vec<u64>` (`PgmIndex::build(data, epsilon)`), predicts a key's position with a
  piecewise linear model and searches within ±ε of the prediction; its README compares
  lookups with `BTreeMap` only (99.7 ns against 291.6 ns per hit on 1M dense keys), with
  no hash map in the comparison and no inserts after the build

### Lookups on the inference path at ac5129c

- `ObjectStrategy` holds an object's properties in an `OrderMap<String, SchemaNode,
  FxBuildHasher>`, whose insertion order is the order of `properties` in the inferred
  schema (genson-core/src/genson_rs/strategy/object.rs:7, 20)
- `ObjectStrategy::add_object` looks each key of an object up in `properties` with
  `get_mut`, and inserts a new `SchemaNode` under an owned copy of the key when the key is
  new to the node (genson-core/src/genson_rs/strategy/object.rs:108-121)
- `ObjectStrategy::add_object` intersects `required_properties` with each object's keys
  through a `KeySet<&str>` collected per object (genson-core/src/genson_rs/strategy/object.rs:122-133)
- Row deduplication in `process_json_strings_parallel` inserts each string's
  `SchemaBuilder::schema_hash` (a u64) into a `HashSet` (genson-core/src/schema.rs:388,
  448, 466)
- No sorted array is searched on the inference path: property lookups hash a string key,
  row deduplication looks a u64 up in a hash set, and the simd-json tape skips a subtree by
  the length its container records

### Method

- rows200 is `bench/claims_rows.py 200` (seed 0): 200 rows, 216.8 MB; labels is the
  `labels` column of bench/data/labels.parquet; narrow200k is 200,000 synthetic rows of six
  fields from `corpora.py` (seed 0)
- Timings are release builds of genson-core/examples/bench_claims.rs on a 4-core Intel
  Xeon @ 2.10GHz VM, binaries alternated in each round, 3 runs per process
- bench_claims at ac5129c gives the schema hashes `ab63cbfae2d3dbf7` (rows200,
  `WIKIDATA=1`) and `b044387a11c95954` (rows200, default options), the hashes recorded in
  2026-10-08-inference-speed.md
- `stage_split.rs` times each row of an NDJSON file on one thread, over two passes, in four
  stages: simd-json `to_tape`, `SchemaBuilder::add_object`, `SchemaBuilder::schema_hash` and
  dropping the builder
- `keypos.py` replays the walk of a fresh builder per row, as `process_json_strings_parallel`
  gives each string, and records where each property lookup finds its key

### Property lookups on rows200

- `keypos.py` on rows200 counts 127,545 objects and 4,620,741 property lookups, of which
  2,602,906 (56.3%) insert a key new to its node
- Of the 2,017,835 lookups of a key its node already holds, `keypos.py` finds 87.83% at the
  slot after the previous key's slot in the same object, 6.22% at a later slot (56.6 slots
  further on average) and 5.94% at an earlier slot
- 1,817,813 of the 2,017,835 lookups of held keys go to nodes of 65 to 1,024 properties
  (the size buckets of `keypos.py`)
- `p1-predicted-slot.diff` compares the key at the slot after the previous key's slot before
  hashing the key in `ObjectStrategy::add_object` — `stage_split.rs` gives 0.995 s of node
  building per pass of rows200 with the patch against 1.017 s at ac5129c, one run each
- bench_claims on rows200 with `p1-predicted-slot.diff` gives medians of 2.11 s wall /
  6.76 s CPU with `WIKIDATA=1` against 2.23 s / 6.86 s at ac5129c, and 1.67 s / 5.54 s with
  default options against 1.69 s / 5.56 s, 9 runs each, with the same schema hashes — the
  CPU ranges of the two binaries overlap (6.24–7.51 s against 6.47–10.93 s, and 5.22–7.05 s
  against 4.96–6.02 s)

### Per-row cost on rows200

- `stage_split.rs` at ac5129c gives, per pass of rows200: 0.548 s in `to_tape`, 1.017 s in
  `add_object`, 0.827 s in `schema_hash` and 0.108 s dropping the builder — `schema_hash`
  33.1% of the four
- The row builders of rows200 hold 2,627,253 nodes, 2,528,300 (96.2%) of them scalar
  leaves (`keypos.py`)
- `BasicSchemaStrategy::hash_schema` feeds a scalar strategy as `S` and the compact
  serialisation of `to_schema()`, which clones the strategy's keywords into a new `Value`
  and inserts `"type"` as a new `String` (genson-core/src/genson_rs/strategy/mod.rs:158-161,
  strategy/base.rs:50-54, node.rs:15-27)
- `SchemaNode::hash_schema` collects each node's written strategies into a `Vec`, which
  `schema_order` partitions into two more, whether the node holds one strategy or several
  (genson-core/src/genson_rs/node.rs:149-156, 208-219)
- docs/journal/2026-10-15-unification-algebra/00_outline.md records item 4 of
  `12_key-interning-and-other-fruit.md` (hashing through `to_schema()`) as wrong since #220,
  which hashes the node tree and calls `to_schema()` for scalar leaves only — the scalar
  leaves are 96.2% of the nodes of rows200's row builders

## Missing

- No timing of bench_claims on a Wikidata chunk file, or of the Python plugin, in this entry
- No count of allocations per stage
