# Working with Parquet files

When your JSON already lives in a Parquet string column, `infer_from_parquet` and
`normalise_from_parquet` read that column and do the work in Rust, without loading the data into a
Polars DataFrame first, which keeps memory use down on large files.

The examples on this page use this file:

```python
import polars as pl

pl.DataFrame({
    "id": ["Q1", "Q2", "Q3"],
    "data": [
        '{"name": "Alice", "tags": ["a", "b"], "labels": {"en": "Alice", "fr": "Alice"}}',
        '{"name": "Bob", "tags": [], "labels": {"de": "Bob"}}',
        None,
    ],
}).write_parquet("input.parquet")
```

## Inferring a schema

`infer_from_parquet` returns the inferred JSON Schema as a dict, or writes it to `output_path`.
It accepts the same inference options as `df.genson.infer_json_schema`.

```python
from polars_genson import infer_from_parquet

schema = infer_from_parquet("input.parquet", column="data", map_threshold=2)
```

```json
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "name": {"type": "string"},
    "tags": {"type": "array", "items": {"type": "string"}},
    "labels": {"type": "object", "additionalProperties": {"type": "string"}}
  },
  "required": ["labels", "name", "tags"],
  "type": "object"
}
```

`labels` has varying keys, so with `map_threshold=2` it is inferred as a map
(`additionalProperties`) rather than a record with fixed fields. Pass `avro=True` to get an Avro
schema instead.

## Normalising to a typed column

`normalise_from_parquet` infers a schema for the column, rewrites every row to match it, and
writes the result to a new Parquet file. With `typed=True`, the output column is a Polars struct
you can read directly:

```python
from polars_genson import normalise_from_parquet

normalise_from_parquet(
    "input.parquet",
    column="data",
    output_path="typed.parquet",
    map_threshold=2,
    typed=True,
    keep_columns=["id"],
)
pl.read_parquet("typed.parquet").unnest("data")
```

```
┌─────┬───────┬────────────┬─────────────────────────────────┐
│ id  ┆ name  ┆ tags       ┆ labels                          │
│ --- ┆ ---   ┆ ---        ┆ ---                             │
│ str ┆ str   ┆ list[str]  ┆ list[struct[2]]                 │
╞═════╪═══════╪════════════╪═════════════════════════════════╡
│ Q1  ┆ Alice ┆ ["a", "b"] ┆ [{"en","Alice"}, {"fr","Alice"… │
│ Q2  ┆ Bob   ┆ null       ┆ [{"de","Bob"}]                  │
│ Q3  ┆ null  ┆ null       ┆ null                            │
└─────┴───────┴────────────┴─────────────────────────────────┘
```

Things to note in this output:

- **Maps become lists of `{key, value}` structs,** because Polars has no map type.
  `typed=True` requires the default `map_encoding="kv"`.
- **Empty arrays and maps become null** (`tags` for Bob). Pass `empty_as_null=False` to keep them
  as empty collections.
- **`keep_columns`** copies the listed input columns (here `id`) into the output file unchanged,
  before the normalised column.
- **Null input rows stay as null output rows** (Q3), so the output has one row per input row and
  lines up with the columns you kept.

## Normalising to JSON strings

Without `typed=True`, the output column holds the normalised rows as JSON strings. This is the
default. The inferred schema is stored in the file's metadata, so you can decode the strings
yourself:

```python
from polars_genson import avro_to_polars_schema, read_parquet_metadata

normalise_from_parquet(
    "input.parquet", column="data", output_path="strings.parquet", map_threshold=2
)
meta = read_parquet_metadata("strings.parquet")
dtype = pl.Struct(avro_to_polars_schema(meta["genson_avro_schema"]))
decoded = (
    pl.read_parquet("strings.parquet")
    .select(pl.col("data").str.json_decode(dtype))
    .unnest("data")
)
```

This gives the same frame as the typed output, minus `id`. The typed output avoids writing every
row out as JSON and parsing it again, so it is faster and needs less memory. The string output is
mainly useful when you need the JSON itself.

## Metadata

Both output forms store two metadata entries, which `read_parquet_metadata` returns as a dict:

- `genson_avro_schema`: the inferred schema, as Avro JSON. `avro_to_polars_schema` converts it to
  a Polars schema.
- `genson_normalise_config`: the normalisation options that were used.

## Large files

- Pass `max_builders` to lower peak memory during inference (see
  [Troubleshooting](get_started.md#memory-usage-exceeds-ram)).
- `output_path` can be the same as `input_path` to overwrite the file in place.
