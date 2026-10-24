# Getting started

polars-genson turns a column of JSON strings into typed Polars columns. It infers a schema from the JSON, then rewrites every row to fit that schema, so rows that differ (missing keys, extra keys, empty arrays, varying object keys) all end up in one consistent set of columns.

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

Here are three rows of JSON that don't agree on their shape: `city` is sometimes missing, `tags` is sometimes empty, and `scores` has different keys in each row.

```python
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

```text
Schema({'id': Int64, 'name': String, 'city': String, 'tags': List(String), 'scores': Map(String, Int64)})
```

`infer_polars_schema` reads every row and reports one Polars schema that fits them all. `scores` becomes a `pl.Map`, because its keys vary from row to row and `map_threshold=1` lets genson treat it as a map rather than a record with fixed fields.

To get the data itself in that shape, use `normalise_json`:

```python
print(df.genson.normalise_json("json", map_threshold=1))
```

```text
shape: (3, 5)
┌─────┬──────┬────────┬────────────┬────────────────────────────┐
│ id  ┆ name ┆ city   ┆ tags       ┆ scores                     │
│ --- ┆ ---  ┆ ---    ┆ ---        ┆ ---                        │
│ i64 ┆ str  ┆ str    ┆ list[str]  ┆ map[str, i64]              │
╞═════╪══════╪════════╪════════════╪════════════════════════════╡
│ 1   ┆ Ada  ┆ London ┆ ["x", "y"] ┆ {"maths": 90}              │
│ 2   ┆ Bob  ┆ null   ┆ null       ┆ {"art": 75, "history": 60} │
│ 3   ┆ Cy   ┆ Oslo   ┆ ["z"]      ┆ {"maths": 80, "art": 70}   │
└─────┴──────┴────────┴────────────┴────────────────────────────┘
```

Every row now has every column. The missing `city` is null, and the empty `tags` list is null too (pass `empty_as_null=False` to keep it as `[]`).

## Where to go next

- [Guides](https://lmmx.github.io/polars-genson/pr-preview/pr-309/guides/index.md) walk through common tasks: inferring a schema, normalising a column, working with Parquet, factoring out repeated data, and large inputs.
- [How it works](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/index.md) explains the decisions genson makes, such as how JSON types map to Polars dtypes and when an object becomes a map.
- The [API reference](https://lmmx.github.io/polars-genson/pr-preview/pr-309/api/polars_genson/index.md) lists every function and option.

## Development install

```bash
cd polars-genson-py/
uv venv && source .venv/bin/activate
uv sync
```
