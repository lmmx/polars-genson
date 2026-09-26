# Behaviours found while writing the docs site

## Current State

- `df.genson.normalise_json(column, decode=schema)` with a `pl.Schema` raises `TypeError: unhashable type: 'Schema'` — the method passes `decode` straight to `str.json_decode(dtype=...)`, which needs a dtype such as `pl.Struct(schema)` (polars-genson-py/python/polars_genson/__init__.py:1226-1229), while the docstring says `decode` accepts a `polars.Schema`.
- `force_field_types={"field": "map"}` replaces the field's schema with `{"type": "object", "additionalProperties": {"type": "string"}}` (genson-core/src/schema/map_inference.rs:321-328), so a forced map always has string values — `{"scores": {"maths": 90}}` normalises to `[{"key": "maths", "value": "90"}]`, while `map_threshold=1` on the same data gives `Int64` values.
- An empty object in a record field normalises to a struct with every field null (`{"meta": {}}` gives `{"k": null}`), with `empty_as_null` either `True` or `False` — `empty_as_null` covers empty arrays and empty maps.
- `infer_json_schema` on the first 50,000 events of GH Archive hour `2024-01-01-12` raised `Genson error: JSON schema inference failed due to invalid JSON input`; whether any of those rows is invalid JSON was not checked.
- `infer_json_schema_from_strings` runs inference inside `panic::catch_unwind` and reports every caught panic with that same "invalid JSON input" message (genson-core/src/schema.rs:599, 658), so the message does not distinguish invalid input from a panic elsewhere in inference.
- On `fix/forced-map-value-type`, a field forced to `"map"` takes its values' common schema, via `forced_map_value_schema` (genson-core/src/schema/map_inference.rs), called from `convert_to_map` (genson-core/src/schema.rs): the schema shared by every value (ignoring nullability), else the values unified by `check_unifiable_schemas` unless a key is in `no_unify`, else `string` — a field that is sometimes a map and sometimes a scalar (a type union) still becomes a map of strings.
- On that branch, `{"scores": {"maths": 90}}` with `force_field_types={"scores": "map"}` normalises to `Int64` values, and the genson-cli snapshots (whose claims fixtures force `labels`, a map of strings, to `"map"`) are unchanged.

## Missing

- Handling of a `pl.Schema` passed as `decode` in `normalise_json`.
- A reproduction of the GH Archive failure narrowed to the rows involved, and an error message that keeps the panic's own text.
