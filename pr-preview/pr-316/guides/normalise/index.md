# Normalise a JSON column into typed columns

`normalise_json` infers a schema for a JSON string column, rewrites every row to fit it, and decodes the result into typed Polars columns.

## Typed columns (the default)

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"json": [
    '{"id": 1, "name": "Ada", "tags": ["x"]}',
    '{"id": 2, "name": "Bo", "email": "bo@example.com"}',
]})
print(df.genson.normalise_json("json"))
```

```text
shape: (2, 4)
┌─────┬──────┬───────────┬────────────────┐
│ id  ┆ name ┆ tags      ┆ email          │
│ --- ┆ ---  ┆ ---       ┆ ---            │
│ i64 ┆ str  ┆ list[str] ┆ str            │
╞═════╪══════╪═══════════╪════════════════╡
│ 1   ┆ Ada  ┆ ["x"]     ┆ null           │
│ 2   ┆ Bo   ┆ null      ┆ bo@example.com │
└─────┴──────┴───────────┴────────────────┘
```

Each field becomes a column, and rows that lack a field get null.

## One struct column: `unnest=False`

To keep the result as a single struct column, pass `unnest=False`. The result is a one-column DataFrame, so it can sit alongside the original columns:

```python
print(df.hstack(df.genson.normalise_json("json", unnest=False).rename({"json": "parsed"})))
```

```text
shape: (2, 2)
┌─────────────────────────────────┬────────────────────────────────┐
│ json                            ┆ parsed                         │
│ ---                             ┆ ---                            │
│ str                             ┆ struct[4]                      │
╞═════════════════════════════════╪════════════════════════════════╡
│ {"id": 1, "name": "Ada", "tags… ┆ {1,"Ada",["x"],null}           │
│ {"id": 2, "name": "Bo", "email… ┆ {2,"Bo",null,"bo@example.com"} │
└─────────────────────────────────┴────────────────────────────────┘
```

## JSON strings: `decode=False`

`decode=False` returns the normalised rows as JSON strings. Every row then has every field:

```python
print(df.genson.normalise_json("json", decode=False).to_list())
```

```text
['{"id":1,"name":"Ada","tags":["x"],"email":null}', '{"id":2,"name":"Bo","tags":null,"email":"bo@example.com"}']
```

## A schema you already have: `decode=schema`

If you already know the schema, for example from `infer_polars_schema` on an earlier batch, pass it as `decode`. genson decodes with it directly and skips inferring the schema for decoding:

```python
schema = df.genson.infer_polars_schema("json")
print(df.genson.normalise_json("json", decode=schema))
```

```text
shape: (2, 4)
┌─────┬──────┬───────────┬────────────────┐
│ id  ┆ name ┆ tags      ┆ email          │
│ --- ┆ ---  ┆ ---       ┆ ---            │
│ i64 ┆ str  ┆ list[str] ┆ str            │
╞═════╪══════╪═══════════╪════════════════╡
│ 1   ┆ Ada  ┆ ["x"]     ┆ null           │
│ 2   ┆ Bo   ┆ null      ┆ bo@example.com │
└─────┴──────┴───────────┴────────────────┘
```

## Options that change the output

- `empty_as_null` (default `True`): empty arrays and maps become null. See [Nulls and empty values](https://lmmx.github.io/polars-genson/pr-preview/pr-316/concepts/nulls-and-empty-values/index.md).
- `coerce_strings`: parse numbers and booleans written as strings. See [Mixed types](https://lmmx.github.io/polars-genson/pr-preview/pr-316/concepts/mixed-types/index.md).
- `map_threshold`, `force_field_types`, `unify_maps`: how objects become maps or records. See [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-316/concepts/maps-and-records/index.md).
- `map_encoding`: how maps are written as JSON, and the dtype they decode to. See [Map types](https://lmmx.github.io/polars-genson/pr-preview/pr-316/concepts/map-types/index.md).
- `wrap_scalars` and `force_scalar_promotion`: how scalars that collide with objects are kept. See [Mixed types](https://lmmx.github.io/polars-genson/pr-preview/pr-316/concepts/mixed-types/index.md).

For data in Parquet files, [`normalise_from_parquet`](https://lmmx.github.io/polars-genson/pr-preview/pr-316/guides/parquet/index.md) does the same without loading the data into a DataFrame.
