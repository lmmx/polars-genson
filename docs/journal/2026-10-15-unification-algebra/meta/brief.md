# genson-core: performance, determinism and structure — agent brief

Repo: `lmmx/polars-genson`, crate `genson-core`. Files referenced are under
`genson-core/src/`. Read the file before editing it; line numbers are from master
2026-10-15 and will drift.

## Ground rules for every work package

- Run `cargo test -p genson-core` (and the Python tests if `polars-genson-py` is touched)
  before and after. No WP may change inferred output unless the WP says so explicitly.
- The existing `insta` snapshots in `genson-cli` and `polars-jsonschema-bridge` are the
  acceptance oracle (see WP0). Every WP ends with `cargo insta test`; no pending
  `.snap.new` is the acceptance criterion unless the WP says otherwise.
- Benchmarks: `just bench` runs `genson-core/benches/schema_inference.rs`;
  `examples/bench_claims.rs` is the wide-key (wikidata claims) case. `bench/` at the
  repo root is the Python suite.
- One WP per PR. Small commits. Do not combine WPs.
- Prefer `&[&Value]` over cloning into `Vec<Value>`; prefer structural `Hash` impls over
  serialising to hash.

Effort scale: **S** = under an hour for an agent, **M** = a few hours, **L** = a day or
more / multi-session. **Serial** = do alone, one agent. **Swarm** = safe to parallelise
across agents (independent files or independent sub-tasks).

---

## Phase 0 — Safety net (do first, light quota)

### WP0 Snapshot coverage — S — serial
Do NOT build a second snapshot system. `genson-cli` already has ~164 `insta` snapshots
plus ~88 verified outputs, and `polars-jsonschema-bridge` ~15. WP0 is: add the wikidata
configuration (`--map-threshold 0 --unify-maps --wrap-scalars`) as a second variant of
the existing insta cases that lack it, and confirm `cargo insta test` is green. The
"snapshot diff" acceptance criterion everywhere below means `cargo insta test` with no
pending `.snap.new` files; `cargo insta accept` is only allowed in WP6/WP7/WP9 with each
accepted diff listed in the PR.

### WP1 Property tests — M — swarm (one agent per property)
Add `proptest` and `jsonschema` (for property 3) to dev-deps. Write a generator for small JSON documents (depth ≤ 4,
≤ 6 keys per object, scalars/arrays/objects/null) and a generator for configs
(`map_threshold ∈ {0,2,20}`, `unify_maps`, `wrap_scalars`, `map_max_required_keys`).
Properties, each its own test and each marked `#[ignore]` if it fails today (record the
failing seed in the test's doc comment — these are bug reports, not blockers):

1. **Row-permutation invariance**: `infer(rows) == infer(shuffle(rows))`.
2. **Chunk invariance**: result independent of `max_builders` and of `GENSON_PARALLEL`.
3. **Soundness**: every input row validates (use `jsonschema` crate) against the inferred
   JSON Schema; every `normalise` output validates against the Avro-derived schema.
4. **Unification is a commutative monoid** (on the values `check_unifiable_schemas`
   accepts): `u(a,b) == u(b,a)`; `u(u(a,b),c) == u(a,u(b,c))`; `u(a,a) == a` modulo
   nullability; and `u` over a list equals `u` over any split of that list.
5. **Hull minimality** (after WP6 only): if a node became a map, no field could have
   remained a record field under the same config.

Expected: 1, 2 and 4 fail today in known ways (see WP5, WP6). Keep the failing seeds.

---

## Phase 1 — Low-risk fixes (output-preserving)

### WP2 Stop cloning in `add_schemas` — S — serial
`genson_rs/strategy/object.rs` ~lines 259, 275, 284: `sub_schemas.iter().map(|&s| s.clone())`
builds `Vec<Value>` because `SchemaNode::add_schemas` (`node.rs` ~333) takes `&[Value]`.
The `SchemaStrategy` trait in `strategy/base.rs` already takes `&[&Value]`, so the change
is to `SchemaNode::add_schemas` and its callers only. Acceptance: snapshot diff empty;
`just bench` (`genson-core/benches/schema_inference.rs`) and `examples/bench_claims.rs`
show merge-phase time not worse. Note `bench/` at the repo root is the Python benchmark
suite, not the Rust one.

### WP3 Cache compiled regexes for patternProperties — S — serial
`object.rs` ~line 120: `Regex::new(p).unwrap()` inside the per-key closure. Store
compiled `Regex` alongside each pattern in `pattern_properties` (e.g. `PropMap<(Regex,
SchemaNode)>` or a parallel `Vec`). Only exercised when a seed schema has
`patternProperties`; add one test that does.

### WP4 Row-dedup hash: 128-bit — S — serial
`schema.rs` ~line 388/448/466 dedups per-row builders on a `u64` (xxh64) from
`chunk_builder.schema_hash()`. A collision silently drops a distinct row schema. Switch
to xxh3_128 (`xxhash-rust`) and a `HashSet<u128>`. The hash is already structural over
the node tree (`builder.rs` ~142, since #220) with a `to_schema()` fallback only for
scalar leaves (`strategy/mod.rs` ~160) — leave that as is. Add a test on the WP1
generator: `schema_hash(a) == schema_hash(b)  <=>  a.to_schema() == b.to_schema()`.
Acceptance: snapshot diff empty; `just bench` unchanged within noise.

---

## Phase 2 — Determinism (may change output; snapshot diffs expected and reviewed)

### WP5 Audit and classify order-dependence — M — serial, read-mostly
Produce `docs/journal/2026-10-15-unification-algebra/meta/determinism.md` (the docs site builds with `strict: true`; `docs/concepts/key-order.md` already covers part of this — extend it rather than duplicate) listing every place inference depends on row order,
key order, or parallel split, with a verdict *intended* / *accidental*:
- `node.rs` `absorb_all` ("each still takes its nodes in order"), `anyOf` branch
  ordering in `node.rs` ~390–440, `add_extra_keywords` first-seen-wins (`object.rs` 54),
  `reorder_unions` in `schema.rs`.
- `map_inference.rs`: `props.values().next()` as the reference schema in
  `identical_record_values` and `unified_values`.
- `unification.rs`: `unify_field_schemas_sequential` vs `_parallel` (promotion not done
  across halves, ~line 433), `unified_anyofs` appended after other fields (~line 512),
  `promotion_field_name`.
Decide: property insertion order is intended; `anyOf` order, extra-keyword winner, and
all of the unification ones are accidental. The failing seeds from WP1 go in this doc.

### WP6 Unification as a commutative monoid — L — serial
`map_inference/unification.rs`. Restructure `check_unifiable_schemas`:
1. **Embed first**: when `wrap_scalars`, map every input through
   `ι(scalar_T) = {type: object, properties: {f__T: scalar_T}}` (using
   `make_promoted_scalar_key` with the existing `promotion_field_name` rule) *before*
   any pairwise step. Delete `try_scalar_promotion`, `try_mixed_scalar_promotion`, and
   the scalar/record branches of `unify_field_pair`; `unify_anyof_schemas` becomes
   "embed, then join".
2. **Join by kind**: all-arrays → join items; all-maps(+empty records) → join values;
   all-records(+empty) → fieldwise join, `required` = intersection; all-scalars of one
   base type → nullable; otherwise `None`. This is what exists; keep it, but as a
   single `join(a, b) -> Option<Value>` that is commutative and associative.
3. **Reduce**: replace `_sequential`/`_parallel` with one
   `schemas.par_iter().cloned().reduce(|| identity, join)` — or a tree reduce over
   `Option` — with no "halves" special case. Identity = the empty record / empty array
   per kind, or handle via `Option`.
4. Dedup inputs by hash before reducing (cheap win; also prepares WP9).
Acceptance: WP1 property 4 passes un-ignored; property 2 passes for unification paths;
snapshot diffs only where WP5 classified the old behaviour as accidental, each diff
reviewed and listed in the PR.

### WP7 Fix accidental merge-phase order dependence — M — serial, after WP5
Per WP5 verdicts: canonicalise `anyOf` branch order (sort by a stable key, e.g. type
tag then structural hash), make `extra_keywords` merge deterministic (or document why
first-wins is intended). Goal: WP1 property 1 passes.

---

## Phase 3 — Structure (memory + speed)

### WP8 Key interning — L — serial (one agent owns the builder)
`genson_rs/strategy/object.rs`, `node.rs`, `builder.rs`.
- Add a `Symbol(u32)` newtype and an interner (`lasso::ThreadedRodeo`, or a per-thread
  `Rodeo` per chunk merged into a global one at chunk join to avoid contention in the
  `par_iter` row loop in `schema.rs` ~407).
- `PropMap<V> = OrderMap<Symbol, V, FxBuildHasher>`; `pattern_properties` likewise.
- `required_properties`: replace `KeySet<String>` with a sorted `Vec<Symbol>` or a
  `FixedBitSet` indexed by symbol; `add_object`'s per-row
  `object.keys().map(|p| p.to_string()).collect()` becomes symbol lookups; `retain`
  becomes a sorted-merge intersection or bitset AND.
- `to_schema` resolves symbols to `&str` once.
- `apply_force_field_types_to_node`, `force_field_types` lookups, `no_unify`: intern
  the config strings once at config construction.
Acceptance: snapshot diff empty; RSS profile lines ("RSS after collecting chunk") drop
on `bench_claims` wide-key inputs; merge phase not slower. Do this *after* WP6 so the
unification code you touch is already the simplified version.

### WP9 Canonical subtree hashes + key partition in map inference — L — serial, after WP8
`map_inference.rs`. Compute a bottom-up 128-bit structural hash per schema node
(nullability-normalised, i.e. hash of the `NonNullView`), cached on the node (if WP10
has not landed, cache in a side `HashMap<*const Value, u128>` or recompute per node —
it's still O(tree) once).
- `identical_record_values`, `unified_values` homogeneity, and
  `debug_unique_schemas` all become hash compares.
- Group a candidate map's keys into classes by hash. Decision table:
  one class → map (as now); all singletons → record; else call WP6's reduce over the
  *distinct* class representatives only.
- Expose the partition in debug output: class count, sizes, representative hash.
- New config `hybrid_maps: bool` (default false): singletons + one large class →
  `properties` for singletons plus `additionalProperties` for the class. JSON Schema and
  Polars (`Struct` + `Map`) only; when `avro=true` and hybrid would fire, fall back to
  current behaviour and log why. Add tests; this is the only WP that adds output modes.
Acceptance: snapshot diff empty with `hybrid_maps=false`; unification profile line on
a wide node (bench a 5k-key map with 3 distinct value shapes) drops by the class ratio.

### WP10 Shape fingerprint before building — L — swarm-friendly (separate fingerprint
    impl from pipeline integration)
`schema.rs` parallel phase. Today every row builds a `SchemaBuilder`, then duplicates
are dropped. Instead:
a. (agent A) A function over the simd-json value that computes a 128-bit *shape
   fingerprint*: recursively hash (sorted key set with each key's child fingerprint,
   scalar type tag, arrays as the *multiset* of item fingerprints — order-insensitive,
   since genson merges items). Must satisfy: equal fingerprint ⇒ equal per-row schema
   under the current builder. Test against WP4b's structural hash on the proptest
   generator: `fp(row_a) == fp(row_b)  ⇒  schema_hash(build(row_a)) == schema_hash(build(row_b))`.
b. (agent B, after A) In the chunk loop, fingerprint every row, build a builder only
   for first-seen fingerprints, keep a count per fingerprint (feeds WP11).
Acceptance: snapshot diff empty; "chunk N parallel" time on `bench_claims` with many
duplicate shapes drops roughly by the duplicate ratio; RSS peak drops.

### WP11 Presence counts in the merge — M — serial, after WP8
`ObjectStrategy`: alongside `required`, keep `seen: u64` on the node and a count per
property (a `Vec<u32>` parallel to the `OrderMap`, or a field on `SchemaNode`). With
WP10 counts, duplicates contribute their multiplicity. Expose
`map_min_required_fraction: Option<f64>` as an alternative to
`map_max_required_keys` (a key is "required" if present in ≥ fraction of rows). No
default behaviour change.

### WP12 Map inference on the typed tree, not on `Value` — L — serial, last
Move `rewrite_objects` to operate on `SchemaNode`/`ObjectStrategy` before `to_schema()`,
so only one tree is alive at peak and the hashes from WP9 live on the node. Only start
this once WP6–WP9 are green; it's a port, not a redesign. Acceptance: snapshot diff
empty; peak RSS on `bench_claims` lower than after WP8.

---

## Phase 4 — Documentation (can run in parallel with Phase 1, light quota)

### WP13 Formal spec in-tree — M — swarm-friendly (one agent, but independent of code)
`docs/concepts/schema-lattice.md` (mkdocs is already set up):
- The schema set `S`, the instance-inclusion order `≤`, nullability (`T ≤ nullable T`).
- Two joins: genson's free join (`anyOf`) vs the Avro join `⊔` (partial; WP6 makes it a
  commutative monoid on its domain). The embedding `ι` for scalar promotion.
- Map inference as the least `Sym(K)`-invariant schema above a record `R = Π_k v_k`
  with required set `Q`; `map_threshold` and `map_max_required_keys` as gates on `|K|`
  and `|Q|`; `force_field_types` as pinning a key as non-interchangeable.
- The key partition by canonical hash and the decision table from WP9.
- `normalise` as the canonical map instance → instance-of-hull.
Link each section to the function implementing it. This doc is the reference the
property tests in WP1 are checking.

---

## Suggested schedule

**Now (light quota, Opus-class agent, serial):** WP0, WP2, WP3, WP4, WP13.
WP0 is the prerequisite for everything; WP2/WP3/WP4 are mechanical; WP13 is prose.

**After reset, serial, in order:** WP1 → WP5 → WP6 → WP7 → WP8 → WP9 → WP11 → WP12.
WP6 and WP8 are the two big ones; do not overlap them.

**Swarm candidates (after reset):**
- WP1: one agent per property (five agents), all writing into one new test file with
  distinct test fns — merge is trivial.
- WP10a and WP10b, plus WP10 vs WP11: fingerprint implementation is independent of the
  builder internals, so it can run alongside WP8 if you accept a rebase.
- WP13 alongside anything.

## What not to do
- Do not introduce orbital graphs, partition backtracking, or any search: schemas are
  trees, every refinement here is a single pass, and a search library adds nothing.
- Do not change default inferred output in any WP except WP6/WP7 (reviewed diffs) and
  WP9 behind `hybrid_maps`.
- Do not use `u64` for any dedup or identity hash on this path.
