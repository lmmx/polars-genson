# Changelog

Release notes for earlier versions are on
[GitHub Releases](https://github.com/lmmx/polars-genson/releases).

## 0.9.0

This release fixes several ways genson silently lost or mistyped data, makes inferred
field order deterministic, and adds `extract_invariants` for JSON that embeds repeated
data.

### Breaking changes

- **Polars ≥ 1.43.2 is required.** genson writes Parquet with `parquet` 60, which marks
  float columns with the IEEE 754 total column order, and Polars before 1.43.2 can't read
  typed output that has a float column.
- **Python ≥ 3.10 is required,** because Polars 1.43.2 requires it.
- **The `polars-lts-cpu` extra is replaced by `rtcompat`**, following Polars:
  `pip install polars-genson[rtcompat]`.
- **`infer_polars_schema` defaults to `avro=True`,** so it reports the dtypes that
  `normalise_json` and typed Parquet output produce. Pass `avro=False` for the old route.

### Output changes

- **A field that is sometimes a scalar and sometimes an object** keeps its scalars under a
  promoted `field__string`-style key by default. See [Mixed types](concepts/mixed-types.md).
- **Integers in a float field** become floats (`1` → `1.0`) instead of null.
- **Arrays that are empty in every row** get the dtype `List(Null)` instead of
  `List(Boolean)`.
- **Field order follows the order keys appear in the JSON,** for objects of any size.
  Objects of more than 32 keys previously gave a different order on each run. See
  [Field order](concepts/key-order.md).
- **JSON Schema to Polars conversion** maps objects with `additionalProperties` to
  `List(Struct{key, value})`, and nullable types to their non-null type.

### New

- **`extract_invariants`** on `normalise_from_parquet`: moves fields that are invariant per
  key into a lookup table before inference. See
  [Factor out repeated embedded data](guides/extract-invariants.md).

### Dependencies

- polars / polars-arrow 0.55.2, pyo3 0.29, pyo3-polars 0.28, arrow / parquet 60.
