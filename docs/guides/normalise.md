# Normalise a JSON column into typed columns

`normalise_json` infers a schema for a JSON string column, rewrites every row to fit it,
and decodes the result into typed Polars columns.

## Typed columns (the default)

```python exec="on" source="above" result="text" session="normalise"
import polars as pl
import polars_genson

df = pl.DataFrame({"json": [
    '{"id": 1, "name": "Ada", "tags": ["x"]}',
    '{"id": 2, "name": "Bo", "email": "bo@example.com"}',
]})
print(df.genson.normalise_json("json"))
```

Each field becomes a column, and rows that lack a field get null.

## One struct column: `unnest=False`

To keep the result as a single struct column, pass `unnest=False`. The result is a
one-column DataFrame, so it can sit alongside the original columns:

```python exec="on" source="above" result="text" session="normalise"
print(df.hstack(df.genson.normalise_json("json", unnest=False).rename({"json": "parsed"})))
```

## JSON strings: `decode=False`

`decode=False` returns the normalised rows as JSON strings. Every row then has every
field:

```python exec="on" source="above" result="text" session="normalise"
print(df.genson.normalise_json("json", decode=False).to_list())
```

## A schema you already have: `decode=schema`

If you already know the schema, for example from `infer_polars_schema` on an earlier
batch, pass it as `decode`. genson decodes with it directly and skips inferring the
schema for decoding:

```python exec="on" source="above" result="text" session="normalise"
schema = df.genson.infer_polars_schema("json")
print(df.genson.normalise_json("json", decode=schema))
```

## Options that change the output

- `empty_as_null` (default `True`): empty arrays and maps become null. See
  [Nulls and empty values](../concepts/nulls-and-empty-values.md).
- `coerce_strings`: parse numbers and booleans written as strings. See
  [Mixed types](../concepts/mixed-types.md).
- `map_threshold`, `force_field_types`, `unify_maps`, `map_encoding`: how objects become
  maps or records. See [Maps and records](../concepts/maps-and-records.md).
- `wrap_scalars` and `force_scalar_promotion`: how scalars that collide with objects are
  kept. See [Mixed types](../concepts/mixed-types.md).

For data in Parquet files, [`normalise_from_parquet`](parquet.md) does the same without
loading the data into a DataFrame.
