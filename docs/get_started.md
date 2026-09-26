# Getting started

polars-genson turns a column of JSON strings into typed Polars columns. It infers a
schema from the JSON, then rewrites every row to fit that schema, so rows that differ
(missing keys, extra keys, empty arrays, varying object keys) all end up in one
consistent set of columns.

## Installation

```bash
pip install polars-genson[polars]
```

On older CPUs, use Polars' compatibility runtime instead:

```bash
pip install polars-genson[rtcompat]
```

polars-genson needs Python 3.10 or later and Polars 1.43.2 or later.

## Quickstart

Here are three rows of JSON that don't agree on their shape: `city` is sometimes
missing, `tags` is sometimes empty, and `scores` has different keys in each row.

```python exec="on" source="above" result="text" session="quickstart"
import polars as pl
import polars_genson  # registers the .genson namespace on DataFrames

df = pl.DataFrame({
    "json": [
        '{"id": 1, "name": "Ada", "city": "London", "tags": ["x", "y"], "scores": {"maths": 90}}',
        '{"id": 2, "name": "Bob", "tags": [], "scores": {"art": 75, "history": 60}}',
        '{"id": 3, "name": "Cy", "city": "Oslo", "tags": ["z"], "scores": {"maths": 80, "art": 70}}',
    ]
})
print(df.genson.infer_polars_schema("json", map_threshold=1))
```

`infer_polars_schema` reads every row and reports one Polars schema that fits them all.
`scores` becomes a list of `{key, value}` structs, because its keys vary from row to row
and `map_threshold=1` lets genson treat it as a map rather than a record with fixed
fields.

To get the data itself in that shape, use `normalise_json`:

```python exec="on" source="above" result="text" session="quickstart"
print(df.genson.normalise_json("json", map_threshold=1))
```

Every row now has every column. The missing `city` is null, and the empty `tags` list is
null too (pass `empty_as_null=False` to keep it as `[]`).

## Where to go next

- [Guides](guides/index.md) walk through common tasks: inferring a schema, normalising a
  column, working with Parquet, factoring out repeated data, and large inputs.
- [How it works](concepts/index.md) explains the decisions genson makes, such as how
  JSON types map to Polars dtypes and when an object becomes a map.
- The [API reference](api/polars_genson.md) lists every function and option.

## Development install

```bash
cd polars-genson-py/
uv venv && source .venv/bin/activate
uv sync
```
