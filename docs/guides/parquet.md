# Normalise JSON stored in Parquet

When your JSON is a string column in a Parquet file, `normalise_from_parquet` reads it,
infers a schema, normalises every row and writes a new Parquet file, all in Rust,
without loading the data into a Polars DataFrame first. This is the fastest route for
large files and uses the least memory.

```python exec="on" session="parquet"
import os, tempfile
os.chdir(tempfile.mkdtemp())  # keep the example files out of the way
```

The examples on this page use this file:

```python exec="on" source="above" session="parquet"
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

## Write typed columns: `typed=True`

With `typed=True`, the output column is a Polars struct you can read directly. Use
`keep_columns` to carry other input columns, such as an `id`, into the output:

```python exec="on" source="above" result="text" session="parquet"
from polars_genson import normalise_from_parquet

normalise_from_parquet(
    "input.parquet",
    column="data",
    output_path="typed.parquet",
    map_threshold=2,
    typed=True,
    keep_columns=["id"],
)
print(pl.read_parquet("typed.parquet").unnest("data"))
```

- `labels` has varying keys, so with `map_threshold=2` it becomes a map, stored as a list
  of `{key, value}` structs (see [Maps and records](../concepts/maps-and-records.md)).
- The empty `tags` list is null (see
  [Nulls and empty values](../concepts/nulls-and-empty-values.md)).
- The null input row stays a null row, so the output has one row per input row and the
  kept `id` column lines up with it.

`typed=True` requires the default `map_encoding="kv"`.

## Write JSON strings instead

Without `typed=True`, the output column holds each normalised row as a JSON string. The
inferred schema is stored in the file's metadata, so you can decode the strings later:

```python exec="on" source="above" result="text" session="parquet"
from polars_genson import avro_to_polars_schema, read_parquet_metadata

normalise_from_parquet(
    "input.parquet", column="data", output_path="strings.parquet", map_threshold=2
)
meta = read_parquet_metadata("strings.parquet")
dtype = pl.Struct(avro_to_polars_schema(meta["genson_avro_schema"]))
decoded = pl.read_parquet("strings.parquet").select(pl.col("data").str.json_decode(dtype))
print(decoded.unnest("data"))
```

This gives the same values as the typed output. Typed output skips writing every row as
JSON and parsing it again, so it's faster and needs less memory; use strings when you
need the JSON itself.

Both output forms store two metadata entries, which `read_parquet_metadata` returns:
`genson_avro_schema` (the inferred schema as Avro JSON) and `genson_normalise_config`
(the options used).

## Only infer the schema

`infer_from_parquet` returns the inferred schema without writing any data. It takes the
same inference options as `df.genson.infer_json_schema`:

```python exec="on" source="above" result="text" session="parquet"
import json
from polars_genson import infer_from_parquet

schema = infer_from_parquet("input.parquet", column="data", map_threshold=2)
print(json.dumps(schema, indent=2))
```

Pass `output_path` to write the schema to a file instead, and `avro=True` for an Avro
schema.

## See also

- [Factor out repeated embedded data](extract-invariants.md), for JSON that carries the
  same nested data in many rows.
- [Process large inputs](large-inputs.md), for memory use on big files.
