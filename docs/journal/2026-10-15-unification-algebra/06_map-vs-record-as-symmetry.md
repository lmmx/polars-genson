> **Note (added 2026-10-15):** Superseded by 08 after reading the code; kept for the record.

Yes — there's a real connection, and it's cleaner than the general "software" cases I listed. Let me build it from your own primitives.

# Map vs record *is* a symmetry question

A JSON object is a function `keys → values`. The question "is this a Map or a Record" is: **does permuting the keys change the schema?**

- **Record**: every key has its own role. Swap `id` and `name` and you get a different (wrong) structure. The group of key-permutations that preserve the schema is trivial.
- **Map**: keys are data. Swap `en` and `fr` and the schema is identical. The group is the full Sym(keys).

So the map/record distinction is literally *which permutation group acts on the key set*, and the schema of a map is the **canonical image** of the object under Sym(keys) — which forgets the keys entirely and keeps only the orbit's value type. `schema_to_dict` ignoring field order is the same move one level up: canonical image of a record under Sym(fields), trivially computed by sorting because fields are a flat set.

Your current `map_threshold` and `map_max_required_keys` are **proxies for the size of that group**. Many keys → probably interchangeable. Few required keys → the ones that always appear are schema, the ones that don't are data. They work, but they're measuring a shadow of the thing.

# The refiner reformulation

Here's what the partition-backtrack/graph-backtracking framing suggests as the direct version.

**Ω = the set of distinct keys** observed at a given path (across all rows, or within one object).

**Start with one cell**: all keys hypothetically interchangeable.

**Apply refiners**, each of which splits cells using one source of evidence, and iterate to fixpoint:

1. **Value-schema refiner** — colour each key by (a canonical hash of) its inferred value subschema. Keys with different value types can't be interchangeable. This is exactly colour refinement with "value schema" as the initial colouring.
2. **Co-occurrence refiner** — build the graph on Ω where `k₁ ~ k₂` if they appear together in a row (weighted by count). Refine by degree profile: keys that co-occur with everything (required-ish) separate from keys that co-occur sporadically. This is your `map_max_required_keys` turned from a global threshold into a per-key colour — and it's structurally the same thing as the 2016 orbital-graph refiner: a graph on the point set whose degree structure tells you which points can't map to each other.
3. **Key-syntax refiner** — colour by lexical class: ISO language code, `P\d+`, ISO date, UUID, snake_case identifier. This is the "fixed structure" constraint: it deliberately *doesn't* commute with arbitrary renaming, which is the point — it encodes prior knowledge that some namespaces are data.
4. **Unification refiner** (your `unify_maps`) — a *coarser* initial colouring: value schema modulo optional fields, i.e. two keys get the same colour if their value schemas have a join in the schema lattice. `no_unify` is a forced split. In the letter example, `{a}` and `{b,c}` are two colours under the strict refiner and one colour under the unified refiner.

**Read the fixpoint partition:**

- One cell → **Map**, values = that cell's (unified) schema.
- All singletons → **Record**.
- Some singletons + one big cell → **Record with `additionalProperties`** — fixed fields plus a map. JSON Schema can express this directly (`properties` + `additionalProperties`/`patternProperties`); Avro can't, but Polars can as `Struct` + `Map` side by side. This hybrid is something thresholds can't produce because they make one global decision per object; the partition gives it to you for free.
- Several big cells → either a record of maps (if the cells are lexically separable) or a map with a union value type (if they're not) — and the decision is now explicit rather than hidden inside a threshold.

The bonus: the partition is an **explanation**. "These keys were classed as a map because they share value schema `{alphabet:int, frequency:float, …}` and co-occur sporadically" is the debug output `debug=True` is reaching for.

# Where Vole's actual machinery does and doesn't apply

Honesty check: JSON values are trees, and canonical forms of trees are linear-time (AHU hashing, which is what "canonical hash of the subschema" already is). You don't need backtracking for the per-object case — refinement alone reaches a fixpoint that *is* the answer. This is the "perfect refiner" regime: pruning without branching.

Where it becomes a genuine graph problem, and where the full framework earns its keep:

- **Nested maps where interchangeability at one level depends on structure at a deeper level.** A Wikidata `claims` object (your parquet examples) is `P-id → [ {mainsnak: {…}, qualifiers: {P-id → […]}, …} ]`. Deciding whether the outer keys are interchangeable requires deciding whether their subtrees are isomorphic *modulo renaming at the inner map levels*. Still a tree, still polynomial — but the group is now a nested (wreath-product-like) thing and you need refinement at all levels simultaneously, which is exactly the "stack of digraphs" architecture.
- **Cross-row key identity.** Once you ask "is key `en` in row 1 the same role as `en` in row 2", keys become vertices connected across rows, and co-occurrence makes it a graph, not a tree. This is where merge-across-rows and map inference are the same computation.
- **Recursive schemas / `$ref`.** Cyclic schema graphs break tree hashing; graph canonicalisation is needed.

# Concrete things this suggests for genson-core

1. Replace (or back) `map_threshold` with a partition-refinement pass; keep the threshold as a fallback for tiny samples where the partition is uninformative.
2. Emit the hybrid `properties + additionalProperties` schema when the partition has singletons plus a big cell.
3. Expose the key partition as the debug/explanation output.
4. Make `unify_maps` a *refiner parameter* (how coarse is the initial colouring) rather than a boolean — e.g. "unify if join exists with ≤ k optional fields."
5. Treat `force_field_types` as pinning a cell (individualisation), the same way backtracking fixes a point.

# Threads

- Work through the Wikidata `claims` shape explicitly and see which refiner does the work at each nesting level.
- The schema-lattice join vs colour refinement: when is "unifiable" transitive, and what breaks if it isn't (it isn't, in general — `a∪b` and `b∪c` can exist while `a∪c` doesn't)?
- Whether co-occurrence-graph refinement subsumes `map_max_required_keys` entirely, or whether required-ness carries information the graph loses.
- Incremental/streaming: can the partition be maintained as rows arrive, the way `merge_schemas` is incremental? Colour refinement is monotone (cells only split), which is promising.
