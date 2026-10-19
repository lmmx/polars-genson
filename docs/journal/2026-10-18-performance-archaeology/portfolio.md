# Opportunity portfolio (living ranking)

Evidence is in the journal entry next to this folder (`../2026-10-18-performance-archaeology.md`)
and in `logs/`. Warm medians on the 4-core VM unless stated; "1T" is one thread. Ranked by
measured avoidable cost on wikidata-pq's path (`normalise_from_parquet`: read, extract
invariants, infer, normalise, write) and on the same data without extraction, against
effort and correctness risk.

## Where the CPU goes, per 200 rows of Wikidata claims

| stage | production shape (label maps extracted) | full rows (no extraction) |
|---|---|---|
| extract_invariants | 1.2–1.5 CPU-s (216 MB in, 4 MB out) | — |
| infer (wikidata-pq options) | 0.3 CPU-s (0.1 s wall) | 6.4–6.7 CPU-s (2.0 s wall) |
| normalise + typed decode (1T) | 0.5–0.8 s | 12–17 s |
| Parquet read / write (1T) | 0.3–0.4 s / 0.03–0.04 s | 0.3–0.4 s / 0.45–0.78 s |
| one-pass bounds | extract: tape parse 0.55 s (1T) | normalise: tape + string appends 0.75 s; arrow-json on the normalised shape 0.52–0.63 s |

The data path (extract, normalise, decode) is 70–75% of the summed stage timings on the
production shape (2.3–2.9 s per 200 rows, Parquet read 13–15%) and ~70% on full rows.

## Ranking

| # | Work package | Measured | Ceiling | Effort | Risk | Issue |
|---|---|---|---|---|---|---|
| 1 | **Map unification's `required` is unsound** | 10,040 / 64,293 objects violate the schema inferred under wikidata-pq's options, on full and slim rows; the Avro schema makes `datavalue` and its fields non-nullable, so the normaliser stores an absent record as a struct of nulls: in the published claims, 1,676 / 1,676 sampled value-less snaks, and absent `latitude`/`precision` stored both ways by chunk. wikidata-pq's schema check cannot see it (Polars dtypes, all-nullable Arrow). Prototype → 0 violations; on wikidata-pq's path, philippesaade and release format, it changes only those absences (49,566 on rows200, 32,935 on a 60-item release chunk), same values, same inferred Polars schema | correctness (published null semantics) | S | 48 snapshots (order kept), all `required` / nullability; output nulls where structs of nulls were | #50 (comment) |
| 2 | **Data path off `serde_json::Value`** (extract, normalise, decode in one pass over the tape) | normalise is 16–21× its floor; extraction is 60% Value parsing + 12% Value equality; #72 move: −11.5% instructions, normalise 7.4 → 4.4 s (1T) | normalise 10×+ (arrow-json 0.55 s on the same output shape), extract ~2× | L (increment S) | normalisation rules, `prune`, lookup equality semantics | #228 (#72) |
| 3 | **Build the map interpretation directly** | record interpretation 20–250× the map interpretation per row (13,136 vs 53 full; 812 vs 40 slim); collapsed full corpus 2.08 → 0.29 s warm (7.1×), RSS −62%; slim spends ~70% of inference on merge, `to_schema` and `rewrite_objects` | inference ≈ parse-bound; `required` correct by construction | L | map decision per path at first sight + verification/fallback; two unification conventions (#110 promotion, nullable optional fields) | #227 |
| 4 | **Small, output-preserving fixes** | lazy debug macros (`rewrite_objects` −22–28%), scalar merge without `to_schema` (merge −20–24%), lazy validation (#194 4c); together rows200 W 2.23 → 1.66 s (−26%), 1.35 s with the purge delay; slim −6% (−17%) | — | S | none known; the purge delay retains memory | #229 |
| 5 | **Fold strings into per-worker builders** | narrow200k 8.6×, labels 4.5×; rows200 and slim200 no faster (per-worker unions of the record interpretation); byte-identical except keyword order on labels | many-small-row workloads | M | keyword order (canonical order: 28 snapshots per 2026-10-08), `processed_count`, forced maps | #230 |
| 6 | **Parquet output is uncompressed** (bytes, not CPU) | `open_writer` uses the default codec, UNCOMPRESSED: rows200's JSON output 212 MB from a 24 MB input, typed 16 MB; zstd 1: typed 5 MB in the same time, JSON 27 MB for +0.2–0.4 s; slim JSON 14.5 → 0.5 MB | 3–29× smaller files | S | file bytes change (data does not); readers must support zstd | #194 (2) |

## Reading of the evidence

- The costs are not in any one function but in representations. Inference carries the
  record interpretation, 20–250× its answer, through every stage before map inference
  collapses it. The data path turns 216 MB of text into `Value` trees two or three times
  (extract parse, normalise parse, normalised output) where one pass over the tape
  suffices. #2 and #3 remove a representation; #4 and #5 make one cheaper.
- #1 is the correctness face of the same late binding: the join that collapses the record
  interpretation after the fact loses optionality. Building the map interpretation (#3)
  gives the right `required` sets by construction.
- Allocation volume (63–129 bytes allocated per input byte) shows up as 27–34% kernel CPU
  on large rows; #2, #3 and #5 cut it at the source, the purge delay only hides it.

## Order of work

1. #1 — correctness, small; first, so that snapshots move once.
2. #4 — small, certain, output-preserving.
3. #2: the #72 move increment, then a schema-compiled tape → Arrow decoder for normalise
   and span-based extraction — largest share of production CPU.
4. #3 — the largest architectural change, after #1 so the collapsed build has a correct
   target.
5. #5 — with a decision on canonical keyword order; for workloads of many small rows.
6. #6 — independent of the rest; a one-line default (or an option) whenever disk or
   transfer matters.

## Killed

- PGM learned index and predicted-slot lookups: no sorted array is searched; lookups are
  not the cost (2026-10-18-schema-hash-and-learned-indexes.md)
- Key interning: tried with lasso and reverted (#133); key strings are not where the
  per-node allocations go
- Allocation-free type checks in `normalise_inner`: −0.1% instructions
- arrow-json schema inference as a normalisation bound: fails on this data (mixed
  object/string `datavalue`); kept as a decoder bound with genson's schema instead
- Fixed-count chunk sizing as its own item: subsumed by #5
- The fold for the production shape: no faster on slim200
- Sequential `ListStrategy` folds (no nested rayon fold for arrays of 10+ items): within
  the run-to-run spread, same hashes; rows200 has 120 such arrays
