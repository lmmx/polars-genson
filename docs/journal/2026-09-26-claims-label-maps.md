# Wikidata claims: embedded label maps and row alignment

Library-side findings from a wikidata-pq review on 2026-09-25/26, measured on one source file (`chunk_1237.parquet`: 10,000 entities, 625 MB of claims JSON) with polars-genson 0.8.0 `typed=True` on a 4-core / 15 GB machine, single runs. The full write-up, including the per-language partition rules, is in wikidata-pq `docs/journal/2026-09-26-claims-label-maps-and-language-rule.md`.

## Current State

- Wikidata claims embed the full multilingual label map of every referenced property, value and unit (`property-labels`, `labels`, `unit-labels`) — on `chunk_1237` those maps total 610.9 MB of 625.3 MB (97.7%), 154,783 maps over 14,257 distinct ids, with one map per id.
- `normalise_from_parquet(typed=True)` on the full `chunk_1237` claims takes 26.3 s (read 3.13 s, infer 10.65 s, parse+normalise+decode 9.18 s, write 3.30 s) at 5.75 GB peak RSS with a 309 MB output.
- `normalise_from_parquet(typed=True)` on `chunk_1237` claims with the three label-map fields removed takes 1.22 s (infer 0.69 s, parse+normalise+decode 0.23 s, write 0.21 s) at 0.39 GB peak RSS with a 3.4 MB output, and infers the full schema minus those three fields.
- A zero-copy serde_json `RawValue` prototype removes the label maps from the 625 MB `chunk_1237` claims in 0.68–0.75 s on 4 threads, keying each map on its sibling `id` / `property` / `unit` field.
- `read_string_column` reads the `chunk_1237` claims column in 3.13 s, against 0.24 s for `pl.read_parquet(columns=["claims"])` over the file's 625 row groups (genson-core/src/parquet.rs).
- `read_string_column` skips null rows in both the `Utf8` and `LargeUtf8` branches (genson-core/src/parquet.rs:79-102) — `normalise_from_parquet` output can hold fewer rows than its input column.
- `normalise_from_parquet` writes only the output column (polars-genson-py/src/parquet_io.rs) — other input columns such as an entity `id` are absent from its output file.
- `polars-arrow` is a dependency of `polars-genson-py`, and the `normalise_json` expression plugin declares a `String` output (polars-genson-py/src/expressions.rs:152-153), while the typed path writes a parquet file that the caller reads back.

## Missing

- An option that extracts subtrees keyed on a sibling field (e.g. `{"labels": "id", "property-labels": "property", "unit-labels": "unit"}`) into a side table and removes them from the rows before inference.
- Null-row preservation in `read_string_column` and `normalise_from_parquet`.
- Carrying selected input columns through `normalise_from_parquet` into the output file.
- A typed Series return from the expression plugin, and a mode that takes a known schema instead of inferring one.
