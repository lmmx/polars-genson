# 2026-10-18 Learned indexes and the cost of `schema_hash`

Whether a learned index (a Piecewise Geometric Model, PGM) applies to genson-core, and the
per-row cost of `SchemaBuilder::schema_hash`, measured along the way. The scripts, the
patches of the two experiments not taken and the raw timings are in
`2026-10-18-schema-hash-and-learned-indexes/` next to this entry.

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

### Hashing scalar leaves without their schema value (branch `perf/schema-hash-scalar-leaves`)

- On the branch, `BasicSchemaStrategy::hash_schema` feeds a scalar strategy without
  keywords as `S`, `{"type":"`, the type name and `"}`, the bytes of `S` and the compact
  serialisation of its schema, without building the schema value; a scalar with keywords
  goes through `to_schema()` as at ac5129c (genson-core/src/genson_rs/strategy/mod.rs:158-170)
- On the branch, `SchemaNode::hash_schema` feeds a node holding one strategy, other than a
  typeless one, as `1` and that strategy's bytes, without the `Vec`s of `schema_order` —
  the bytes the general path feeds for such a node (genson-core/src/genson_rs/node.rs:208-217)
- `test_hash_schema_values_are_unchanged` pins the xxh64 that `hash_schema` gives over 2000
  random batches, a fifth of them with a schema of keyworded scalars added, and one
  `SchemaBuilder::schema_hash` value, both as at ac5129c (genson-core/src/tests/node.rs)
- `test_scalar_hash_is_its_serialised_schema` compares the bytes a scalar strategy feeds
  with `S` and `serde_json::to_writer` of its schema, for the five scalar types bare and
  two scalars with keywords (genson-core/src/tests/node.rs)
- `test_hash_schema_tells_keyworded_scalars_apart` checks that a `format` keyword
  separates a string's hash from the bare string's and from another `format`'s, alone and
  as a property, and that `integer` and `number` hash apart (genson-core/src/tests/node.rs)
- The three tests pass at ac5129c (commit 8b59f30, before the change) and on the branch
- `p2-tagged.diff` (against ac5129c) feeds a bare scalar as `s` and its type name, a byte
  stream of its own, with the same `SchemaNode::hash_schema` change — under `p2-tagged.diff`,
  `test_hash_schema_values_are_unchanged` and `test_scalar_hash_is_its_serialised_schema`
  fail and `test_hash_schema_matches_schema_equality` passes
- `stage_split.rs` on rows200, three runs of each build alternated, gives medians per pass
  of 0.871 s in `schema_hash` at ac5129c (0.833–0.898 s), 0.320 s on the branch
  (0.315–0.331 s) and 0.306 s with `p2-tagged.diff` (0.293–0.316 s); `to_tape` and
  `add_object` stay within 3% of ac5129c, and the drop takes 0.105 s and 0.104 s against
  0.112 s (timings.txt)
- bench_claims, 5 rounds of 3 runs per build (n=15, `run_e2e.sh`, timings.txt), medians
  of wall / CPU at ac5129c, on the branch, and with `p2-tagged.diff`:
  - rows200 `WIKIDATA=1`: 2.31 s / 7.12 s, 2.04 s / 6.34 s, 2.12 s / 6.32 s
  - rows200 default options: 1.75 s / 5.68 s, 1.49 s / 4.72 s, 1.57 s / 5.03 s
  - labels: 0.170 s / 0.570 s, 0.140 s / 0.480 s, 0.150 s / 0.500 s
  - narrow200k: 0.840 s / 2.700 s, 0.710 s / 2.270 s, 0.720 s / 2.300 s
- The CPU medians on the branch are 11.0% (rows200 `WIKIDATA=1`), 16.9% (rows200 default),
  15.8% (labels) and 15.9% (narrow200k) under those of ac5129c, and the CPU ranges of the
  two builds overlap in all four cases (timings.txt)
- An earlier round of the comparison, with `p1-predicted-slot.diff` applied as well (n=9),
  gave rows200 CPU medians of 6.45 s against 6.86 s with `WIKIDATA=1` and 5.07 s against
  5.56 s with default options
- Every run of one corpus and set of options gives the same schema hash at ac5129c, on the
  branch and with `p2-tagged.diff`
- `process_json_strings_parallel` hashes a builder per string only with 10 or more strings
  (`PARALLEL_THRESHOLD`, genson-core/src/schema.rs:22) — genson-cli reading a file without
  `--pq-column` passes one string, so the sequential path calls no `schema_hash`
- On the branch, `cargo test -F avro` in genson-core (160 passed, 2 ignored), `cargo test`
  in genson-cli (181 passed, 11 ignored, no snapshot changes) and in polars-jsonschema-bridge
  (37 passed), `cargo clippy -D warnings` on genson-core (all targets, `avro`) and
  genson-cli, and `cargo fmt --check --all` pass
- On the branch, `maturin develop` (debug build) and `pytest tests/` in polars-genson-py
  give 262 passed and 3 skipped
- `peak_rss.py` reads each bench_claims process's `ru_maxrss` with `os.wait4` (the
  counter `/usr/bin/time -v` reports as "Maximum resident set size"), one inference per
  process, 5 rounds with the builds alternated; medians at ac5129c against the branch
  (timings.txt):
  - rows200 `WIKIDATA=1`: 1057.6 MB (1029.8–1062.1) against 1071.5 MB (1050.5–1099.0)
  - rows200 default options: 1067.5 MB (1056.7–1079.2) against 1061.4 MB (1050.7–1066.6)
  - labels: 61.4 MB (59.4–64.5) against 63.2 MB (59.6–64.3)
  - narrow200k: 47.4 MB (47.4–47.6) against 47.4 MB (47.3–47.5)
- bench_claims holds the input rows as `String`s for the whole run, 216.8 MB of the peak
  RSS for rows200 in both builds (genson-core/examples/bench_claims.rs)
- `count_alloc.py` puts a counter of allocations and requested bytes (`alloc`,
  `alloc_zeroed`, `realloc`) around mimalloc in genson-core/src/genson_rs/mod.rs, for
  measurement only, and `alloc_count.rs` reads the counter around the stages of
  `stage_split.rs` (one pass, one thread) and around one `infer_json_schema_from_strings`
  call; each build of the measurement has a target directory of its own — two worktrees
  built into one target directory gave one binary for both, which cargo took as fresh
- `alloc_count.rs` on rows200, ac5129c against the branch (timings.txt):
  - `schema_hash` stage: 18,120,375 allocations (1,245,384,410 bytes) against 149,536
    (45,945,824 bytes)
  - `to_tape` stage: 1,400 allocations in both; `add_object` stage: 19,279,899 against
    19,269,486, the arrays of 10 or more items merging on rayon in both
  - one inference with `WIKIDATA=1`: 67,566,011 allocations (14,885,107,348 bytes) against
    49,804,369 (13,754,084,218 bytes)
- `alloc_count.rs` on narrow200k, one inference with default options: 21,958,454
  allocations (2,311,360,746 bytes) at ac5129c against 10,623,671 (1,592,864,635 bytes)
  on the branch
- Every `alloc_count.rs` run of one corpus and set of options gives the same schema hash on
  both builds

## Missing

- No timing of bench_claims on a Wikidata chunk file, or through the Python plugin
