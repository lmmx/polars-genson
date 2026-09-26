# Mixed scalar types lose all but one type on normalisation

## Current State

- A field that holds an integer in some rows and a string in others infers as `{"type": ["integer", "string"]}` in JSON Schema — `pl.DataFrame({"j": ['{"x": 1}', '{"x": "a"}']}).genson.infer_json_schema("j")`.
- `reorder_unions` orders union branches by a fixed rank with `integer` (11) before `string` (14) (genson-core/src/schema.rs:84, 146, 149), so the Avro union for `x` has `int`/`long` as its first non-null branch.
- `normalise_value` normalises a union against its first non-null branch (genson-core/src/normalise.rs:295), and the `int`/`long` arm maps a non-numeric string to `null` unless `coerce_string` is set (genson-core/src/normalise.rs:154).
- `normalise_json("j")` on that frame returns `['{"x":1}', '{"x":null}']` — the string `"a"` is dropped without an error, with or without `unify_maps=True`, on master after #201.
- polars-genson 0.3.0, 0.5.8, 0.7.10 and 0.8.1 all return `['{"x":1}', '{"x":null}']` for the same frame.
- `infer_polars_schema("j")` reports `x: Int64` through the Avro route and `x: String` through the JSON Schema route.
- Mixed-scalar promotion (`x__integer` / `x__string`) exists in `try_mixed_scalar_promotion` (genson-core/src/schema/map_inference/unification.rs:909), called only while unifying map values or records (unification.rs:628) — a plain record field with a scalar type union does not reach it.

## Missing

- Promotion or another lossless representation for record fields whose rows hold different scalar types.
- A test for a field that is an integer in some rows and a string in others.
