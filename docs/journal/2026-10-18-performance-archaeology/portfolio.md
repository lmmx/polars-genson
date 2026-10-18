# Opportunity portfolio (living ranking)

Evidence is in the journal entry next to this folder (`../2026-10-18-performance-archaeology.md`)
and in `logs/`. Numbers are warm medians on the 4-core VM unless stated; "1T" means one
thread. Ranked by measured avoidable cost on the production path (wikidata-pq's
`normalise_from_parquet`: read, infer, normalise, write) × generality, against effort and
correctness risk.

## Where the production path's CPU goes (rows200, 216.8 MB of claims)

| stage | cost | evidence |
|---|---|---|
| infer (W options, 4 threads) | 2.0 s wall, 6.4–6.7 CPU-s | wave1_time, e4_fold_v2 |
| normalise, typed (1T) | parse 1.0–1.6 s + normalise 6.5–11.7 s + `values_to_struct_array` 4.1 s ≈ 12–17 CPU-s | e7_normalise |
| floor for normalising: simd-json tape + one walk appending every string to Arrow (1T) | 0.75–0.80 s | e9_norm_floor |

Normalisation is about two thirds of the measured CPU, and sits 16–21× above its floor.

## Ranking

| # | Work package | Measured | Ceiling | Effort | Risk | Issue |
|---|---|---|---|---|---|---|
| 1 | **Map unification's `required` is unsound** | 10,040 / 64,293 objects violate the inferred schema under wikidata-pq's own options; Avro makes 20 `datavalue` fields non-nullable that no row fills together; prototype → 0 | correctness | S | output changes (a fix); snapshot churn | #50 |
| 2 | **Normalise straight into the output** (no parsed and normalised `Value` trees) | typed normalise ≈ 12–17 CPU-s / 200 rows; 31% inserting into output `IndexMap`s, 12% SipHash, 12.5% drops | floor 0.75 s: 5–8× after real-world overheads (estimate) | L | normalisation rules (promotions, maps, coercions, prune) | new |
| 2a | ↳ increment: move fields out instead of cloning (#72) | −11.5% instructions of the whole run (20 rows), normalise 7.4 → 4.4 s median 1T, same output | — | S | prune path keeps the clone | #72 |
| 3 | **Build the map interpretation directly** (bind map-ness early) | record interpretation 13,136 nodes/row (213,685 merged) vs 53/row (81) for the answer; collapsed corpus 2.08 → 0.29 s warm (7.1×), 3.38 → 0.29 s cold, RSS 1.2 → 0.46 GB; map paths decided by the first row | inference ≈ parse-bound (0.26 s with fold) | L | needs map paths before the build + verify/fallback; two unification conventions to reproduce (#110 integer/number, nullable optional fields) | new |
| 4 | **Fold strings into per-worker builders** (no per-string trees, hashes, chunks, merge) | narrow200k 0.94 → 0.11 s (8.6×), labels 0.143 → 0.032 s (4.5×), rows200 CPU −16–19% at equal wall; byte-identical where keyword order is unaffected | — | M | keyword order (labels) unless a canonical order is adopted (28 snapshots, 2026-10-08); `processed_count`, forced maps | new |
| 5 | **Four small, output-preserving fixes** | lazy debug macros: `rewrite_objects` −22–28%; scalar merge fast path: merge −20–24%; lazy validation (#194 4c): prep −90%, total −8–21%; purge delay: −11–20% wall on large rows (RSS +10%) | ~30–40% of inference on rows200 W together (not measured together) | S each | none known (purge: retained memory) | new (links #194) |

## Reading of the evidence

- The costs are not in any one function but in the size and number of intermediate
  representations. Inference carries the record interpretation (250–2,600× the answer)
  through every stage before map inference collapses it; normalisation materialises the
  data three times (parsed `Value`, normalised `Value`, Arrow) where one suffices. #3 and
  #2 remove a representation; everything else makes a representation cheaper.
- #1 is the correctness face of the same late-binding design: the join that collapses the
  record interpretation after the fact loses optionality. Building the map interpretation
  directly (#3) gives the correct `required` sets with no fix at all.
- #4 is a loss only under the record interpretation, whose union each worker rebuilds;
  under the map interpretation the fold costs 0.262 s against 0.296 s.
- The kernel's share (27–34% of CPU on large rows) is a consequence of allocation volume
  (63 bytes allocated per input byte); #3 and #4 cut it at the source, the purge delay
  only hides it.

## Order of work

1. #1 (correctness, small) — before any change that moves output, so snapshots move once.
2. #5 (small, certain, output-preserving) — independent of everything else.
3. #2a, then #2 (largest share of production CPU).
4. #4 with a decision on canonical keyword order.
5. #3 (largest architectural change; #1 first, so the collapsed build has a correct target).

## Killed

- PGM learned index and predicted-slot lookups (P1): no sorted array is searched;
  lookups are not the cost (2026-10-18-schema-hash-and-learned-indexes.md)
- Key interning: tried with lasso and reverted (#133); key strings are not where the
  per-node allocations go
- Allocation-free type checks in `normalise_inner`: −0.1% instructions
- arrow-json as a normalisation bound: cannot infer this data's schema (mixed
  object/string `datavalue`)
- Fixed-count chunk sizing as its own item: subsumed by #4 (folding needs no chunks)
