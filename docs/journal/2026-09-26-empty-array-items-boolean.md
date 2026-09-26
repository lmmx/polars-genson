# Arrays that are always empty get List(Boolean)

## Current State

- A field whose arrays are empty in every row infers as `{"type": "array", "items": {}}` in JSON Schema — the items have no type.
- The Avro conversion (avrotize 0.1.1) turns untyped items into its generic union `["null", "boolean", "int", "long", "float", "double", "bytes", "string", array, map]` (avrotize-0.1.1/src/common/generic.rs:5-30).
- `avro_type_to_polars_type` converts a union to its first non-null branch (polars-jsonschema-bridge/src/deserialise.rs:174-176) — the generic union's first non-null branch is `boolean`, so the field becomes `List(Boolean)`.
- `normalise_value` also normalises a union against its first non-null branch (genson-core/src/normalise.rs:295).
- On `pl.DataFrame({"json_col": ['{"empty_array": []}', '{"empty_array": []}']})`, `normalise_json("json_col").schema` and `infer_polars_schema("json_col")` (Avro route) both give `empty_array: List(Boolean)`, and the JSON Schema route gave `List(String)` — polars-genson 0.8.1.
- The column holds only nulls (with `empty_as_null=True`) or empty lists (with `empty_as_null=False`), so no values are lost — the dtype is `Boolean` with no data behind it.
- A field that is empty in some rows and non-empty in others takes its item type from the non-empty rows, and is unaffected.

## Missing

- A dtype for always-empty arrays that does not assert a value type, e.g. `List(Null)`.
- A test for an array field that is empty in every row.
