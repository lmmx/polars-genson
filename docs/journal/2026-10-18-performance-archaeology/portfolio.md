# Opportunity portfolio (living ranking)

Ranked by measured avoidable cost × generality ÷ (effort × correctness risk). Evidence
lives in the journal entry next to this folder; numbers are warm medians on the 4-core
VM unless stated. Status: **promoted** (issue), **open** (being tested), **killed**.

| # | Opportunity | Measured ceiling | Workloads | Effort | Risk | Status |
|---|---|---|---|---|---|---|
| 1 | Map unification marks optional fields required | 10,040 / 64,293 objects violate the inferred `required` (rows200 W); prototype → 0 | every `unify_maps` run | S | output changes (fixes) | open |
| 2 | Build the map interpretation instead of the record interpretation | rows200 W 2.08 s → 0.29 s (7.1× warm, 11.6× cold), RSS 1.2 → 0.46 GB | map-heavy configs (the Wikidata use case) | L | high: needs map paths before the build | open |
| 3 | Per-string trees, hashes, chunks and merge, versus folding | narrow200k 10.8×, labels 3.7×; rows200 slower (record interpretation) | many-row workloads | M | output order semantics | open |
| 4 | Page faults from allocator purging | rows200 W −20% wall (1.99 → 1.60 s) | large rows | S | retained memory | open |
| 5 | Fixed-count chunks (`max_builders`) | narrow200k 3.5× at 10,000 | small rows | S | memory bound per chunk | open (subsumed by 3) |
| 6 | Redundant validation in `prepare_json_bytes` | ≤0.38–0.51 thread-s / run on rows200 W | all parallel runs | S | error messages | open |

## Reading of the evidence so far

- The dominant cost is not any one function: it is the size of the intermediate
  representation. On the Wikidata configuration every stage before `rewrite_objects`
  carries a tree 248× (per row) to 2,638× (merged) larger than the map interpretation that
  is the answer, and allocation, page faults, merge, `to_schema` and map inference all
  scale with it. #1 is the correctness consequence of the same late-binding design: the
  join that collapses the record interpretation loses optionality.
- #3 and #2 interact: folding is a loss only because the record interpretation's union is
  large. Under the map interpretation a fold would be cheap.

## Killed

- (see journal "Measured and not kept" sections as they accrue)
