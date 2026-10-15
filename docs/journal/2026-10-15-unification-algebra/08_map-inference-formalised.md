I'll read the actual code rather than reason from the README. Cloning the repo.

I read `map_inference.rs`, `map_inference/unification.rs`, and the call site in `schema.rs`. You were right to push back: my previous message described a partition-refinement algorithm that the code does not contain. Here's what it actually does, formalised, and then what the group-theory vocabulary does and doesn't buy.

# What the code does

**Stage 1 (genson-rs merge).** Each row is parsed into a per-row schema; rows are merged by genson's node merge: properties are unioned, `required` is intersected, incompatible types become `anyOf`. By the time `rewrite_objects` runs, you have one merged tree and per-row information is gone except for the `required` set (= keys present in every row).

**Stage 2 (`rewrite_objects`).** Top-down walk. At each object node with `properties`, in order:
- field-name overrides (`force_field_types`, `force_scalar_promotion`, `force_parent_field_types`);
- `anyOf` at this node → attempt `unify_anyof_schemas`;
- always-empty object with `map_threshold == 0` → map of null;
- the heuristic gate: `|props| ≥ map_threshold` and `|required| ≤ map_max_required_keys`;
- `identical_record_values` fast path (all value schemas literally equal records);
- `unified_values`: all values equal modulo nullability → map of that; else if `unify_maps` → `check_unifiable_schemas` over the values; else stay a record and recurse into fields.

The decision at a node is **all-or-nothing over the key set**: either every key becomes a map key or every key stays a field. There is no partition of keys. `debug_unique_schemas` counts the distinct value schemas, which *is* the partition's class count, but it's debug-only.

**Stage 3 (`check_unifiable_schemas`).** A typed, partial binary operation on schemas, folded over the list:

- `array ⊔ array = array(items₁ ⊔ items₂)`; an array with no `items` is the identity
- `map ⊔ map = map(values₁ ⊔ values₂)`; empty records are identities for maps
- `record ⊔ record` = fieldwise `⊔` on the union of fields; `required` = fields present in *all* inputs; others made nullable
- `scalar_T ⊔ scalar_T = nullable T`; `null ⊔ X = nullable X`
- with `wrap_scalars`: `scalar_T ⊔ record = record ⊔ {f__T: scalar_T}` (promotion embeds the scalar as a one-field record first), and `scalar_T ⊔ scalar_U = {f__T, f__U}`
- everything else: undefined (`None`)

Folding is sequential pairwise (`unify_field_pair`) below 50 schemas, otherwise split in halves in parallel — with an explicit note that promotion doesn't happen *across* halves.

# The formal structure

Let **S** be the set of schemas. The genson merge is a total join in a lattice where `anyOf` is the free (coproduct) join. Unification `⊔` is a **partial join on the Avro-expressible sublattice**: it tries to collapse what genson would express as `anyOf` into a single record/map/array, with nullability as the only "slack" (`T ≤ nullable T`). Where no such collapse exists, `⊔` is undefined.

A record node is `R = Π_{k∈K} v_k` with a required subset `Q ⊆ K`. The symmetric group `Sym(K)` acts on instances of `R` by renaming keys. `Map(V)` is the type of finite partial functions from *any* key set into `V`; it is the type that is invariant under *all* key renamings. So:

> **Map inference is a closure operator: it replaces `R` by the least schema above `R` that is `Sym(K)`-invariant.** Without `unify_maps` that hull exists iff all `v_k` are equal modulo nullability. With `unify_maps` the hull is `Map(⊔_k v_k)` when the join exists.

The information discarded by the hull is exactly the stabiliser `Stab_{Sym(K)}(R) = Π_c Sym(K_c)`, where the `K_c` are the classes of keys with equal value schema (further split by membership in `Q`). The thresholds are proxies: `map_threshold` ≈ "`|K|` is large enough that forgetting it is worth it"; `map_max_required_keys` ≈ "`Q` is small, so the keys aren't acting like field names". The "required" check and the homogeneity check are two different refiners (one by presence, one by value type), applied as global gates rather than as a partition.

`normalise` is then the canonical map instance ↦ instance-of-hull; `schema_to_dict` is canonical form of records under `Sym(fields)` at every node, which is trivial because fields are a flat set (sort them).

# What the refiner/backtracking literature does and does not buy

Honest answer first: **the search machinery buys nothing.** Schemas are trees; every "refiner" here is perfect in the JSC-2023 sense (one pass of value-colouring decides the stabiliser, no branching). There is no orbital-graph speedup to be had, because there is no backtracking to prune. The transfer is at the level of *vocabulary and design discipline*, and three of the design consequences are also algorithmic wins:

**1. Make `⊔` total, associative and commutative.** Right now it's partial and (because promotion happens inside the pairwise step) order-dependent — the "no promotion across halves" note documents a case where a sequential fold would succeed and the parallel fold fails. Fix: apply promotion as a *canonical embedding* `ι: scalar_T ↦ {f__T: scalar_T}` to every input first (when `wrap_scalars`), then join in the record sublattice, where the join is genuinely a commutative monoid (fieldwise join, intersection of required sets). Once `⊔` is a commutative monoid, unification is a parallel reduce with no caveats and is trivially *incremental* across chunks — the same reason genson's merge is incremental.

**2. Canonical hashing of subschemas.** `NonNullView::eq` and `identical_record_values` are structural comparisons over `serde_json::Value`, O(n·|schema|) per node. A bottom-up canonical hash (AHU-style: hash of sorted (field, child-hash) pairs, with nullability normalised) computed once per node makes homogeneity O(n), makes the key partition free (group by hash), and gives you a stable identity for "this value type" that persists across rows and chunks. This is the tree-case canonical image.

**3. Expose the key partition and act on it.** With hashes, the classes `K_c` cost nothing. Their shape is informative even when the hull doesn't exist:
   - one class → map (what you do now);
   - all singletons → record;
   - singletons + one big class → record of the singletons plus `additionalProperties` for the big class — JSON Schema expresses this directly; Polars as `Struct` + `Map`; Avro can't, so it'd be a JSON-Schema-only output or a named split;
   - two big classes with no join → currently "not a map, keep n fields" (or `string` under force). With the partition you can at least report *why*, and optionally emit a map of `anyOf`.
   This is also the right `debug` output: "map because 1 class of 412 keys with schema `#a1f3…`", "record because classes {id},{name},{…}".

**4. Graded required-ness.** `Q` is a boolean per key because the merge intersects. If the merge kept a presence count per key per node (cheap — one integer per property), `map_max_required_keys` becomes a threshold on a distribution rather than on an exact intersection, which is more robust to one bad row, and the two refiners (value-type, presence) become colours on the same key set.

**5. Where the digraph framework would become literally necessary:** recursive schemas / `$ref`, or any attempt to canonicalise across rows by key identity rather than by position. Neither is in the code now, and I'd leave them out.

# What I'd document in the module, mathematically

- The lattice: define `S`, `≤` (instance-set inclusion), nullability, the free join (`anyOf`) vs the Avro join (`⊔`), and the embedding `ι` for promotion. State associativity/commutativity as a property with a test.
- Map inference as the `Sym(K)`-invariant hull; thresholds as gates on `|K|` and `|Q|`; `force_*` as pinning (individualisation, in the backtracking idiom — "this key is fixed, not interchangeable").
- The key partition `K = ⊔ K_c` by canonical hash, with the decision table above.
- Normalisation as the canonical map to the hull.

# Threads

1. Write down `⊔` as a monoid and find every place the current implementation is order-sensitive (promotion, `anyOf` ordering, first-schema-wins choices like `props.values().next()`).
2. The hybrid record+map output: which downstream (Polars, Avro, Parquet) can take it, and how `normalise` would split a row.
3. Presence counts in the merge: cost in genson-rs's node structure and what `map_max_required_keys` becomes.
4. Whether a canonical hash should be nullability-blind (match `NonNullView`) or carry nullability as a separate bit, and what that does to class boundaries.
