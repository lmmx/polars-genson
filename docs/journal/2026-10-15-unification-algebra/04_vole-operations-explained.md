# What Vole actually is

One sentence: **Vole is a solver for questions about relabellings.** A permutation of Ω is a relabelling of a set of identifiers. A permutation group is "the set of relabellings that don't matter." Every operation Vole offers is some combination of three questions:

1. **Find all** relabellings satisfying some constraints → you get a *group* (or a *coset* of one).
2. **Find one** → an isomorphism / matching, or a proof none exists.
3. **Find the canonical one** → a normal form you can hash and compare.

That's it. The entire API is "give me constraints, tell me which question." The interface (roughly — I'm reconstructing from the manual) is a list of constraints, each built from a *structure* plus an *action* describing how relabellings act on that structure:

- `Constraint.Stabilise(X, action)` — relabellings that leave X unchanged.
- `Constraint.Transport(X, Y, action)` — relabellings that turn X into Y.
- `Constraint.InGroup(G)` / `InCoset(C)` — restrict to a known family of relabellings. This is where the 2016 orbital-graph refiners live: how do you prune a search by "must be in G" without enumerating G? Answer: refine by G's invariant digraphs.
- `Constraint.Normalise(H)`, `Centralise(h)`, `Conjugate(H, K)` — the meta-level ones (below).

Then `Vole.FindGroup(constraints)`, `Vole.FindOne(constraints)`, `Vole.FindCanonical(G, constraints)`, with convenience wrappers `Vole.Stabiliser`, `Vole.Intersection`, `Vole.Normaliser`, `Vole.CanonicalImage`, `Vole.CanonicalPerm`.

The **action** argument is the recognition hook for software: `OnPoints`, `OnSets`, `OnTuples`, `OnSetsSets`, `OnTuplesSets`, `OnDigraphs`. It says what your data *is* under renaming. A set is order-free. A tuple is positional. A set of sets is a hypergraph. A digraph is anything with references. Internally all of these become labelled digraphs on a stack — the point of the 2019 paper is that one representation and one refinement procedure (colour refinement) handles them all uniformly.

# The operations, translated

**Stabiliser = automorphism group = "what are the symmetries of this thing."** Given a graph/schema/structure X, which renamings of its identifiers give back X unchanged? Software reading: *which identifiers in this structure are interchangeable?* Two columns in a table that are structurally indistinguishable. Two nodes in a dependency graph with identical roles. Two function parameters that are used symmetrically. Knowing the stabiliser tells you where you can avoid redundant work: if a search over configurations of X is symmetric under its automorphisms, explore one representative per orbit.

**Transport = isomorphism = "is X the same as Y up to naming."** Finds a renaming g with Xᵍ = Y or proves none exists. Software reading: structural equality where names are arbitrary. Are these two ASTs the same program modulo local variable names? Are these two JSON schemas equivalent modulo field order and gensym'd `$ref` ids? Are these two query plans the same? A hidden detail: Transport with a `InGroup(G)` constraint means *only some* renamings are allowed — you can rename locals but not fields, reorder commutative args but not non-commutative ones. That's the realistic software case and it's exactly what nauty can't do and Vole can.

**Intersection = "satisfy several constraints at once."** Stabiliser of X *and* in G *and* maps Y to Z. Relabellings that preserve the type structure and the control flow and the data flow. Every real software equivalence is an intersection of several invariances, and the pruning power comes from refining on all of them together rather than solving one and filtering.

**Coset intersection** — same, but when one constraint is "maps X to Y" rather than "fixes X." The result is either empty or `g·(stabiliser)`: one witness plus the full group of alternatives. Useful when you need not just "are they equivalent" but "give me every way they're equivalent" (e.g. all valid alignments between two structures).

**Canonical image = "pick a deterministic representative of the equivalence class."** Given X and allowed relabellings G, return the lexicographically-least Xᵍ (and `CanonicalPerm` returns the g). This is the operation with the widest software footprint, because it turns an *equivalence* into an *equality*: you can then hash, dedup, memoise, cache, and diff. Any time you've written "sort the keys before hashing" or "normalise before comparing," you've hand-rolled a canonical image for a very easy group. Vole does it for groups where sorting doesn't work — where the structure is cyclic, or references itself, or where only some renamings are permitted.

**Normaliser = "symmetries of the symmetry structure."** N_G(H) = relabellings in G that map the *set* H to itself (not pointwise — they may permute H's elements among themselves). Software reading: suppose you've found that a structure has invariance group H. Which further relabellings *respect that pattern of invariance* as a whole? Example: a dataset where certain column groups are interchangeable (H). The normaliser tells you which renamings preserve "these groups of columns are interchangeable" — permuting whole equivalence classes consistently. It's the operation for reasoning about equivalences-of-equivalences, which is where schema migration and refactoring live.

**Conjugacy = "are these two symmetry groups the same up to renaming."** Is there g with Hᵍ = K? Two systems with the same *shape* of invariance even if their identifiers differ.

**Centraliser = "relabellings compatible with a given transformation."** C(h) = {g : gh = hg}. Software reading: h is some fixed operation (a rotation of a ring buffer, a shift, a cyclic permutation of a pipeline's stages); the centraliser is every renaming that doesn't break h — the *equivariant* relabellings. If you're building something that must commute with a known transformation, the centraliser is the set of things you're allowed to do.

# Signals that one of these is hiding in your code

Each of these is a permutation group action you haven't named:

- Tests that flake with HashMap/dict iteration order → your structure's equality is up to a group, and you're comparing non-canonical representatives.
- IDs that are arbitrary: gensyms, UUIDs, node ids, register numbers, temp variable names, column aliases, auto-numbered constraints → the group is "renamings of these ids," and anything that should be stable across runs needs a canonical image.
- Candidate generation that produces duplicates "that are really the same" → dedup via canonical image under the right group, or generate orbit representatives directly (orderly generation).
- Two representations you *believe* are equivalent but check by eyeballing → Transport.
- Work that's repeated for configurations that are "obviously symmetric" → Stabiliser, then search orbits.
- A transformation that must preserve some invariant structure → Normaliser/Centraliser.
- A data structure with cycles or shared references where Merkle-hashing fails → canonical form under automorphisms before hashing.

# Why this beats what you'd do by hand

The naive approach is "try renamings and check." Vole's approach is: represent the structure as a coloured digraph stack, run colour refinement (each identifier gets a colour from its own colour plus the multiset of its neighbours' colours, iterate to fixpoint) — identifiers that end up with distinct colours *cannot* be interchanged, so the search space collapses to "which same-coloured identifiers map to which." Then branch on one ambiguous identifier, refine again, recurse. Every constraint gets to contribute refinement, and the group constraint contributes via its orbital graphs. For most real structures refinement does nearly all the work and the search tree is tiny. The "perfect refiners" paper is about when the tree is literally one node.

Practically: Vole is a GAP package with a Rust core talking to GAP over a pipe; it's not a drop-in library yet. The transferable thing is the architecture — the digraph-stack-plus-refiner design is quite portable, and the paper is explicit enough to reimplement for a specific domain.

# Questions to locate your data-processing case without naming it

1. In your data, which identifiers are *arbitrary* (could be renamed with no change of meaning) and which are *fixed*? The arbitrary ones are Ω; the constraints on how they may be renamed are G.
2. Is the core problem *comparison* (Transport), *dedup/caching* (canonical image), *skipping redundant work* (Stabiliser + orbits), or *preserving a known invariance while transforming* (Normaliser/Centraliser)?
3. Is the structure tree-shaped (sorting works, you don't need this), DAG-shaped (Merkle works, probably don't need this), or cyclic/shared-reference/multi-relation (you need this)?
4. Are there several independent invariances that have to hold simultaneously? That's the Intersection case, and where refiners most outperform filter-after-solve.
5. Do you need the canonical form to be *stable across versions of the data* (append a record, canonical form of the old part shouldn't change)? That's a genuinely harder requirement and worth examining, since lexicographic canonical images are not incremental.

Pick whichever of these bites and I'll go deep on that one.
