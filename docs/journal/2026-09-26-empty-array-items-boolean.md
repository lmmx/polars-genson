# Arrays that are always empty get List(Boolean)

## Current State

- A field whose arrays are empty in every row infers as `{"type": "array", "items": {}}` in JSON Schema — the items have no type.
- The Avro conversion (avrotize 0.1.1) turns untyped items into its generic union `["null", "boolean", "int", "long", "float", "double", "bytes", "string", array, map]` (avrotize-0.1.1/src/common/generic.rs:5-30).
- `avro_type_to_polars_type` converts a union to its first non-null branch (polars-jsonschema-bridge/src/deserialise.rs:174-176) — the generic union's first non-null branch is `boolean`, so the field becomes `List(Boolean)`.
- `normalise_value` also normalises a union against its first non-null branch (genson-core/src/normalise.rs:295).
- On `pl.DataFrame({"json_col": ['{"empty_array": []}', '{"empty_array": []}']})`, `normalise_json("json_col").schema` and `infer_polars_schema("json_col")` (Avro route) both give `empty_array: List(Boolean)`, and the JSON Schema route gave `List(String)` — polars-genson 0.8.1.
- The column holds only nulls (with `empty_as_null=True`) or empty lists (with `empty_as_null=False`), so no values are lost — the dtype is `Boolean` with no data behind it.
- A field that is empty in some rows and non-empty in others takes its item type from the non-empty rows, and is unaffected.
- `twitter.json` from the simdjson examples (all 100 statuses) has always-empty `entities.symbols` and `retweeted_status.entities.symbols`, and `citm_catalog.json` has always-empty `performances[].seatCategories[].areas[].blockIds` — each file is itself a sample, so a field empty there need not be empty in the full source.
- Five Wikidata claims chunks (`chunk_1-00021`, `chunk_2-00066`, `chunk_2-00108`, `chunk_2-00114`, `chunk_3-00031`) have no always-empty array fields, under the wikidata-pq options and under defaults.
- `pl.concat(how="vertical_relaxed")` of a `List(Boolean)` column with a later batch's `List(Struct)` column raises `SchemaError`, and of a `List(Null)` column with the same `List(Struct)` column gives `List(Struct)` — polars 1.44.2. Plain `pl.concat` (`how="vertical"`) raises `SchemaError` for both.
- On `fix/empty-array-null-dtype`, `SchemaInferenceResult::to_avro_schema` rewrites untyped array items (`"items": {}`) to `{"type": "null"}` before avrotize runs (genson-core/src/schema/core.rs), so an always-empty array converts to Avro `{"type": "array", "items": "null"}` and to Polars `List(Null)` — `infer_json_schema` output keeps `"items": {}`.
