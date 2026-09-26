# Scalar values dropped from scalar/object unions under default options

## Current State

- A field that holds a scalar in some rows and an object in others infers as `anyOf[object, scalar]` in JSON Schema — `pl.DataFrame({"j": ['{"value": "plain"}', '{"value": {"amount": 3, "unit": "kg"}}']}).genson.infer_json_schema("j")` gives `value: anyOf[{amount, unit}, string]`.
- `normalise_json` on that frame with default options returns `{"value": {"amount": null, "unit": null}}` for the first row — the string `"plain"` is dropped without an error or warning, on polars-genson 0.8.1.
- The same frame with `unify_maps=True` returns `{"value": {"amount": null, "unit": null, "value__string": "plain"}}` for the first row — the string is kept under a promoted `value__string` field.
- Scalar promotion of an `anyOf[object, scalar]` field runs in `unify_anyof_schemas` (genson-core/src/schema/map_inference/unification.rs), and both call sites that reach it are gated on `config.unify_maps` (genson-core/src/schema/map_inference.rs:135, 354) — with the default `unify_maps=False`, the `anyOf` stays unpromoted.
- `wrap_scalars` defaults to `True`, and its docstring says scalars are promoted "when they appear in contexts where other rows provide objects" (polars-genson-py/python/polars_genson/__init__.py:168-171) — under default options that promotion does not happen for `anyOf[object, scalar]` fields.
- `normalise_value` handles an Avro union by normalising against the first non-null branch (genson-core/src/normalise.rs:295) — for a `[record, string]` union, a string row is normalised as a record and every record field becomes null.
- `force_scalar_promotion` wraps a field whose schema is a plain scalar type (`"type": "string"` etc.) and does not handle `anyOf[object, scalar]` (genson-core/src/schema/map_inference.rs:227-260) — passing `force_scalar_promotion={"value"}` does not keep `"plain"` either.
- The Wikidata claims pipeline in wikidata-pq passes `unify_maps=True`, so `datavalue` scalars there are promoted to `datavalue__string` and are not affected.
- Every released version tested (0.2.6, 0.3.0, 0.4.7, 0.5.0, 0.5.8, 0.6.0, 0.6.8, 0.7.0, 0.7.4, 0.7.10, 0.8.1) drops `"plain"` under default options; `unify_maps=True` keeps it as `value__string` from 0.3.0 onwards and drops it in 0.2.6.
- genson merges the object branches of a field into one object schema before promotion runs — `{"v": "s"}`, `{"v": {"a": 1}}`, `{"v": {"b": "x"}}` with `wrap_scalars=False` infers as `anyOf[{a, b}, string]` — so promoting the scalar without `unify_maps` (#201) adds `v__string` to that one object and merges no further objects.

## Missing

- Scalar promotion of `anyOf[object, scalar]` fields when `wrap_scalars=True` and `unify_maps=False`.
- A test covering a scalar/object union under default options.
