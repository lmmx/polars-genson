# Outline

A chat with Claude Fable, 2026-10-15. Odd-numbered files are Louis's messages and
even-numbered files are Fable's replies. The chat begins with permutation group search
in general and moves to genson-core from 05 onwards.

08 and 12 were written after Fable read the genson-core source on GitHub master. That
source matches 96ef9d5: 12 cites `absorb_all`, added in #220 (703f140), and no file under
`genson-core/src` changed between 703f140 and 96ef9d5.

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
  acts on the key set, and proposes a partition-refinement pass over keys. Superseded:
  08 states that this algorithm does not appear in the code, and a note at the top of
  06 says so.
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
  paragraph on each, and property tests with proptest as the first step. The gains are
  estimates with no measurements behind them; `genson-core/examples/bench_claims.rs` is
  the benchmark meant to replace them.
- `11_key-interning-question.md` — asks about formalising key interning, and about
  other speed, correctness and memory gains.
- `12_key-interning-and-other-fruit.md` — written after reading the vendored genson-rs
  builder (`strategy/object.rs`, `node.rs`). Proposes `u32` key symbols, and lists other
  changes by effort. Ends with an order: 3 → 4 → 1/2 → interning → 5 → 6.
  - Item 3 is the one correctness bug in the list, not an optimisation: rows are
    deduplicated on a u64 xxh64 hash (genson-core/src/schema.rs:388-466,
    genson_rs/builder.rs:142), so a collision drops a distinct row schema without an
    error. It became WP4 in `meta/brief.md`.
  - Item 4 is wrong: since #220, `schema_hash` hashes the node tree and calls
    `to_schema()` only for scalar leaves (genson_rs/strategy/mod.rs:160).

## meta/

- `meta/brief.md` — an agent brief from Fable, written after reviewing this outline.
  Splits the avenues from 08, 10 and 12 into 14 work packages (WP0–WP13) in four phases:
  safety net, output-preserving fixes, determinism, and structure, plus an in-tree spec.
  Gives each one an effort size, whether it can run in parallel, and acceptance
  criteria, and ends with a schedule and a "What not to do" list that rules out search
  and backtracking.
- `meta/response.md` — Fable's reply to the outline and to four corrections to the
  first brief (WP0 reuses the existing insta snapshots, WP2 changes
  `SchemaNode::add_schemas` not the trait, WP4 drops the structural-hash step, and
  benchmarks are `just bench` and `bench_claims`, not `bench/`). Accepts the corrections,
  orders WP0, WP2, WP3, WP4 for today, and suggests the outline changes made here. The
  first version of the brief and of this note is in git history (b31901a).
  - The note says #220 postdates the code Fable read. It does not: see the paragraph at
    the top of this outline.
