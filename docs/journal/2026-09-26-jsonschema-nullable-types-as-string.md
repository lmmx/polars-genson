# JSON Schema route converts nullable fields to String

## Current State

- genson writes a nullable field in JSON Schema as a type array, e.g. `"timezone": {"type": ["null", "integer"]}` — seen in the JSON Schema inferred for `genson-cli/tests/data/claims/x1818_L4.jsonl` with `map_threshold=0, unify_maps=True, force_field_types={"mainsnak": "record"}`.
- `json_type_to_polars_type` matches on `type_value.as_str()` only, so a type array takes the `None => Ok("String")` fallback (polars-jsonschema-bridge/src/deserialise.rs:71-72, 105) — every nullable field converts to `String` whatever its non-null type.
- On `x1818_L4.jsonl` with those options, 292 fields convert to `String` through the JSON Schema route and to `Int64` through the Avro route (e.g. `datavalue.timezone`, `datavalue.precision`, `datavalue.before`, `datavalue.after`) — polars-genson 0.8.1 built from `feat/infer-polars-schema-avro-default`.
- `infer_polars_schema` uses the Avro route by default from `feat/infer-polars-schema-avro-default`, so the fallback reaches `infer_polars_schema` only with `avro=False`; direct callers of `json_type_to_polars_type` and `schema_to_polars_fields(.., SchemaFormat::JsonSchema, ..)` reach it always.

## Missing

- Handling of `type` arrays in `json_type_to_polars_type`: a `["null", T]` array converts as `T`.
- A policy for type arrays with more than one non-null type (e.g. `["integer", "string"]`).
- A unit test for a nullable scalar and a nullable object.
