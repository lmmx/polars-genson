# extract_lookup: moving repeated per-id subtrees into a lookup table

## Current State

- Every Wikidata claim embeds the full multilingual label map of each entity it references, as a `language → label` map next to the referenced id: `labels` next to `id` in `datavalue`, `property-labels` next to `property` in each snak, `unit-labels` next to `unit` in quantity values.
- On `chunk_2-00114` (4,095 rows, 93.6 MB of claims JSON), the three label-map fields total 61.4 MB (65.6%), across 13,571 occurrences and 1,288 distinct `(field, key)` pairs (829 `labels`, 451 `property-labels`, 8 `unit-labels`).
- On `chunk_2-00114`, every `labels`/`property-labels`/`unit-labels` object has its sibling key (`id`/`property`/`unit`), and no key maps to two different subtrees.
- `property-labels` occurs in `mainsnak` (5,929 times on `chunk_2-00114`) and in qualifier and reference snaks (e.g. under `P248`, `P143`, `P813`), so the fields sit at several depths.
- A review session on 2026-09-25/26 measured `chunk_1237` (10,000 rows, 625 MB of claims JSON): label maps 97.7% of the bytes, and `normalise_from_parquet(typed=True)` at 26.3 s / 5.75 GB peak RSS on full claims against 1.22 s / 0.39 GB with the maps removed — recorded in `2026-09-26-claims-label-maps.md`.
- `normalise_from_parquet` reads the JSON column, infers a schema from every row, normalises every row against it, and writes one output file (`output_path`), with `keep_columns` copied alongside (polars-genson-py/src/parquet_io.rs).
- "Normalise" in genson means making each row conform to the inferred schema, so the database sense of the word (factoring repeated data into a separate table) is not available as a name.
- A survey of comparable tools found no dedicated term for removing a keyed subtree from rows and storing each distinct one once: normalizr calls it "normalize" into "entities", JSON:API calls the pattern "sideloading" (`included`), star schemas call the result a dimension or reference table, and dlt/Airbyte/Singer "child tables" / "unnesting" split nested data out without deduplicating by key.

## Missing

- An `extract_lookup: dict[str, str]` option on `normalise_from_parquet`, mapping a field name to its sibling key field (e.g. `{"labels": "id", "property-labels": "property", "unit-labels": "unit"}`), under which each object holding both fields has the field removed before inference and normalisation.
- A `lookup_output_path` option naming the Parquet file that receives one row per distinct `(field, key)`, with the columns `field`, `key` and `value`.
- A representation for `value` in the lookup table: a JSON string (any subtree shape) or typed columns.
- A policy for a key that appears with two different subtrees: an error, or the first subtree kept with a count of conflicts.
- A round-trip test: re-inserting each lookup `value` next to its key in the slim rows reproduces the original rows, on the genson-cli claims fixtures and on one Wikidata chunk.
- Timing and peak-RSS measurements of `normalise_from_parquet` with and without `extract_lookup` on a Wikidata chunk.
- `extract_lookup` on `normalise_json`, `infer_json_schema` and `infer_from_parquet`.
- Deduplication of lookup rows across files.
