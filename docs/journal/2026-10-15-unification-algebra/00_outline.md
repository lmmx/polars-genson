# Outline

A chat with Claude Fable, 2026-10-15. Odd-numbered files are Louis's messages and
even-numbered files are Fable's replies. The chat begins with permutation group search
in general and moves to genson-core from 05 onwards.

- `01_refiners-applications-question.md` — asks where the partition-backtrack refiner
  work (Jefferson, Pfeiffer, Waldecker, "New refiners for permutation group search")
  applies to software, and asks for the provenance and later history of the papers in
  some 2016–2019 tweets.
- `02_refiners-provenance-and-software-uses.md` — traces the line from Leon's partition
  backtrack through orbital-graph refiners to graph backtracking, Vole and "perfect
  refiners". Lists software uses (canonical forms of IRs, AC rewriting, symmetry in model
  checking, hashing cyclic data) and explains the polyhedral-symmetry links from the
  tweets.
- `03_what-vole-does-question.md` — asks for Vole's operations in more detail, to find
  where they could apply.
- `04_vole-operations-explained.md` — explains Vole as a solver for questions about
  relabellings, and gives a software reading of each operation (stabiliser, transport,
  intersection, canonical image, normaliser, centraliser). Lists signs that a group
  action is hiding in code.
- `05_json-map-unification-question.md` — points Fable at map unification in genson-core
  and polars-genson.
- `06_map-vs-record-as-symmetry.md` — frames map versus record as which permutation group
  acts on the key set, and proposes a partition-refinement pass over keys. 08 states that
  this algorithm does not appear in the code.
- `07_did-you-read-the-code.md` — questions whether 06 matches what the code does, and
  points to `genson-core/src/schema/map_inference.rs`.
- `08_map-inference-formalised.md` — written after reading `map_inference.rs`,
  `map_inference/unification.rs` and `schema.rs`. Describes the three stages (genson
  merge, `rewrite_objects`, `check_unifiable_schemas`) and formalises `⊔` as a partial
  join and map inference as the `Sym(K)`-invariant hull. Finds no use for the
  backtracking machinery, and proposes five avenues: a total commutative `⊔`, canonical
  subschema hashing, the key partition, presence counts, and a written spec.
- `09_expected-gains-question.md` — asks what each avenue would achieve (speed, memory,
  correctness, code size).
- `10_expected-gains-per-avenue.md` — a table of expected gains per avenue, with a
  paragraph on each, and property tests with proptest as the first step.
- `11_key-interning-question.md` — asks about formalising key interning, and about
  other speed, correctness and memory gains.
- `12_key-interning-and-other-fruit.md` — written after reading the vendored genson-rs
  builder (`strategy/object.rs`, `node.rs`). Proposes `u32` key symbols, and lists other
  changes by effort, including a u64 row-dedup hash that drops a row's schema on a
  collision. Ends with an order: 3 → 4 → 1/2 → interning → 5 → 6.

## meta/

- `meta/brief.md` — an agent brief from Fable, written after reviewing this outline.
  Splits the avenues from 08, 10 and 12 into 14 work packages (WP0–WP13) in four phases:
  safety net, output-preserving fixes, determinism, and structure, plus an in-tree spec.
  Gives each one an effort size, whether it can run in parallel, and acceptance
  criteria, and ends with a schedule and a "What not to do" list that rules out search
  and backtracking.
- `meta/response.md` — Fable's note sent with the brief. Names WP0 (snapshot corpus)
  and WP10 (shape fingerprint before building) as the two additions beyond the chat,
  advises WP0, WP2, WP3 and WP13 on a low quota, and says "What not to do" encodes the
  correction made in 08.
