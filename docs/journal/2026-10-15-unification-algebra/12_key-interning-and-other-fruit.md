I'll check how keys are actually stored in the vendored genson-rs builder before opining.

Yes — and the code confirms it's worth doing. Here's what I found in the builder (`genson_rs/strategy/object.rs`, `node.rs`) and `schema.rs`, in rough order of value.

# Key interning

**Current state.** `PropMap<V> = OrderMap<String, V, FxBuildHasher>`: every property key is an owned `String` per node. Each row gets its own `SchemaBuilder` in the parallel phase, so a key like `mainsnak` or `P31` is allocated once per row per level it occurs at, then moved during `absorb`. `required_properties: KeySet<String>` holds a *second* owned copy of each key per node, and the `add_object` fast path allocates `object.keys().map(|p| p.to_string())` for every row to seed it. So allocation count is roughly rows × keys × 2, for a vocabulary that is typically tiny.

**Proposal.** Intern to a `u32` symbol (a `lasso`-style table; thread-local rodeos merged at chunk join avoids contention). Consequences:
- `PropMap<Symbol, SchemaNode>`: hashing a key is one integer op, equality is one compare, no allocation on the hot path.
- `required` becomes a bitset (or sorted `Vec<Symbol>`) over symbols; per-row intersection is an AND rather than building a `KeySet<&str>` and calling `retain`. Presence counts (avenue 4) are then a `Vec<u32>` indexed by symbol — nearly free.
- Memory: one copy of each distinct key process-wide. Per-row builders shrink enough that the "freeing duplicates is a large share of the serial merge" comment (and the parallel `drop`) stops being necessary.
- The key partition and canonical hashing from earlier become integer-indexed, which is what makes them cheap.
- Cost: resolve symbols back to strings in `to_schema`; insertion order is preserved because `OrderMap` is positional.

I'd rate this: memory **large**, speed **medium** (merge phase), correctness neutral, moderate refactor contained to the builder.

# Other fruit, by effort

**Low effort, clear win**
1. `add_schemas` in `object.rs` clones every sub-schema `Value` (`sub_schemas.iter().map(|&s| s.clone())`, three times) just to pass a `&[Value]` where `&[&Value]` would do. Pure waste on the merge path.
2. The pattern-properties path calls `Regex::new(p)` inside the per-key closure — recompiling every regex for every key of every object. Cache compiled regexes. Rarely hit, but when it is, it's pathological.

**Medium effort, correctness**
3. Row dedup hashes with a **u64** (`seen_hashes.insert(hash)`), and a collision silently drops that row's schema contribution — a distinct shape vanishes with no error. At millions of rows the birthday bound is not comfortable. Use a 128-bit hash (xxh3-128) or verify on hit. This is the one I'd fix first because it's a silent-wrong-output bug.
4. `schema_hash` computes `self.to_schema()` and serialises the whole `Value` just to hash it (strategy/mod.rs:160 via `hash_value`). That materialises a full schema tree per row and throws it away. Implement `Hash` structurally on the strategy tree instead — no `Value`, no serialisation. Same output, one allocation-heavy pass removed per row.

**Higher effort, structural**
5. **Shape fingerprint before building.** For workloads with millions of rows and few distinct shapes, the biggest win is not building per-row builders for duplicates at all: a single pass over the simd-json tape computing a structural hash of (keys, scalar types, recursively; arrays hashed order-insensitively since items merge) gives a fingerprint equivalent to the per-row schema hash. Build a builder only for a novel fingerprint. This changes the parallel phase from "build everything, drop duplicates" to "hash everything, build the distinct few". It's the algorithmic analogue of canonical-image dedup, and it subsumes 3–4.
6. **Map inference on the typed tree, not on `Value`.** Today: `to_schema()` → `Value` → `rewrite_objects` walks and clones `Value`s → Avro/Polars conversion. Two full trees alive at peak, string-keyed, with deep `==` compares. Running inference on `SchemaNode` (symbol-keyed, with cached subtree hashes) before serialising removes one tree and makes the hashing/partition avenues natural. It's the right home for the "canonical hash per node" idea.

**Ordering sensitivity, again.** `absorb_all`'s comment that "each still takes its nodes in order" and `add_extra_keywords`' first-seen-wins mean the merge, not just unification, is order-dependent (keyword values, `anyOf` branch order). The property test "inference is invariant under row permutation" will fail today; deciding which order-dependences are intended (insertion order of properties, probably yes) and which are accidental (`anyOf` ordering, extra keywords) is worth doing before the structural work, since symbols and fingerprints both assume a canonical answer exists.

If you want a single sequence: 3 → 4 → 1/2 → interning → 5 → 6, with the property tests written before interning so the refactors have a net.