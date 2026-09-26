# Integers dropped from float fields on normalisation

## Current State

- A field holding `1` in one row and `1.5` in another infers as `{"type": "number"}` in JSON Schema and `x: Float64` in Polars — `pl.DataFrame({"j": ['{"x": 1}', '{"x": 1.5}']})`.
- The `double`/`float` arm of `normalise_value` kept a number only when `n.is_f64()` (genson-core/src/normalise.rs:165) — a JSON integer is not an `f64` in serde_json, so it fell through to `null`.
- `normalise_json("j")` on that frame returns `['{"x":null}', '{"x":1.5}']` on polars-genson 0.3.0, 0.5.8, 0.7.10 and 0.8.1.
- The wikidata-pq claims schema carries both `latitude__integer` and `latitude__number` (likewise `longitude`, `precision`) through `force_scalar_promotion`, which keeps integer and float values in separate columns.
- On `fix/normalise-integer-in-float-field`, the arm widens any JSON number to a float, so `1` becomes `1.0`.
- A promoted record assigns a scalar to every `__`-suffixed field whose type matches (genson-core/src/normalise.rs, record arm) — widening alone put an integer into both `precision__integer` and `precision__number`, which changed 5 genson-cli claims snapshots from `precision__number: null` to the integer as a float.
- On `fix/normalise-integer-in-float-field`, a promoted record routes an integer to its `__integer`/`__int`/`__long` field when it has one and to its `__number`/`__float`/`__double` field otherwise, and a float to its float field — the genson-cli claims snapshots are unchanged.

## Missing

- A check of whether wikidata-pq's `latitude`/`longitude`/`precision` promotion is still needed once integers survive in float fields.
