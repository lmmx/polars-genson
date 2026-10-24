# Normalise JSON stored in Parquet

When your JSON is a string column in a Parquet file, `normalise_from_parquet` reads it, infers a schema, normalises every row and writes a new Parquet file, all in Rust, without loading the data into a Polars DataFrame first. This is the fastest route for large files and uses the least memory.

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

## Write typed columns: `typed=True`

With `typed=True`, the output column is a Polars struct you can read directly. Use `keep_columns` to carry other input columns, such as an `id`, into the output:

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
print(pl.read_parquet("typed.parquet").unnest("data"))
```

```text
shape: (3, 4)
┌─────┬───────┬────────────┬────────────────────────────────┐
│ id  ┆ name  ┆ tags       ┆ labels                         │
│ --- ┆ ---   ┆ ---        ┆ ---                            │
│ str ┆ str   ┆ list[str]  ┆ map[str, str]                  │
╞═════╪═══════╪════════════╪════════════════════════════════╡
│ Q1  ┆ Alice ┆ ["a", "b"] ┆ {"en": "Alice", "fr": "Alice"} │
│ Q2  ┆ Bob   ┆ null       ┆ {"de": "Bob"}                  │
│ Q3  ┆ null  ┆ null       ┆ null                           │
└─────┴───────┴────────────┴────────────────────────────────┘
```

- `labels` has varying keys, so with `map_threshold=2` it becomes a map, stored as a Parquet `MAP` and read back as a `pl.Map` (see [Map types](https://lmmx.github.io/polars-genson/pr-preview/pr-301/concepts/map-types/index.md)).
- The empty `tags` list is null (see [Nulls and empty values](https://lmmx.github.io/polars-genson/pr-preview/pr-301/concepts/nulls-and-empty-values/index.md)).
- The null input row stays a null row, so the output has one row per input row and the kept `id` column lines up with it.

`typed=True` takes `map_encoding="mapping"` (the default) or `"kv"`, which stores maps as lists of `{key, value}` structs.

With `unnest=True`, the struct's fields are written as top-level columns after the `keep_columns`, as `DataFrame.unnest` makes them (a null row is null in each column), so the file reads back without the `.unnest("data")`. A kept column with the same name as one of the struct's fields is an error.

## Write files in a shared schema: `schema=`

Files normalised one at a time but read together, such as chunks of a large dataset or shards processed in parallel, need one schema between them. Each file's inferred schema is only what its own rows hold: a nested field no row of a chunk has is missing from that chunk's file, and a field null in every row has the dtype `Null`. Pass the dataset's schema as `schema`, and every file is written in it, however little of it a file's rows use.

genson still infers each file's schema, and uses it twice:

- it checks it against yours before writing, so a file holding data your schema has no place for (a new field, a string where you have an integer) stops with an error naming the field, rather than losing the data or writing another schema;
- it keeps it in the file's metadata, so you can see what each file actually held.

The rows are put into your schema's shape as they are written, in the same pass.

Take two chunks of one dataset. No row of `chunk_a` has a `postcode` or a `score`:

```python
pl.DataFrame({
    "id": ["a1", "a2"],
    "data": [
        '{"name": "Alice", "address": {"city": "Paris"}, "score": null}',
        '{"name": "Ana", "address": {"city": "Lisbon"}, "score": null}',
    ],
}).write_parquet("chunk_a.parquet")

pl.DataFrame({
    "id": ["b1", "b2"],
    "data": [
        '{"name": "Bob", "address": {"city": "Rome", "postcode": "00100"}, "score": 3}',
        '{"name": "Bea", "address": null, "score": 7}',
    ],
}).write_parquet("chunk_b.parquet")
```

`schema` is the output column's fields, as a `pl.Schema` (or a `pl.Struct`), and needs `typed=True`:

```python
stored = pl.Schema({
    "name": pl.String,
    "address": pl.Struct({"city": pl.String, "postcode": pl.String}),
    "score": pl.Int64,
})

for chunk in ["a", "b"]:
    normalise_from_parquet(
        f"chunk_{chunk}.parquet",
        column="data",
        output_path=f"out_{chunk}.parquet",
        typed=True,
        keep_columns=["id"],
        schema=stored,
        unnest=True,
    )

print(pl.read_parquet("out_*.parquet"))
```

```text
shape: (4, 4)
┌─────┬───────┬──────────────────┬───────┐
│ id  ┆ name  ┆ address          ┆ score │
│ --- ┆ ---   ┆ ---              ┆ ---   │
│ str ┆ str   ┆ struct[2]        ┆ i64   │
╞═════╪═══════╪══════════════════╪═══════╡
│ a1  ┆ Alice ┆ {"Paris",null}   ┆ null  │
│ a2  ┆ Ana   ┆ {"Lisbon",null}  ┆ null  │
│ b1  ┆ Bob   ┆ {"Rome","00100"} ┆ 3     │
│ b2  ┆ Bea   ┆ null             ┆ 7     │
└─────┴───────┴──────────────────┴───────┘
```

Both files have the schema's columns and dtypes, its fields in its order at every depth: `chunk_a`'s `address.postcode` is null in each row, and its `score`, `Null` in its inferred schema, is written as the schema's `Int64`. A null value stays null (`Bea`'s `address`).

### Which inferred schemas fit: `schema_check`

The inferred schema is checked against `schema` before anything is written. With `schema_check="compatible"` (the default) it fits when, recursively:

| inferred                | fits the schema's                                                          |
| ----------------------- | -------------------------------------------------------------------------- |
| `Null` (no value seen)  | any dtype                                                                  |
| a dtype                 | same dtype (no widening: `Int64` does not fit `Float64`)                   |
| a struct                | struct holding each of its fields, each fitting (the schema may have more) |
| a list, an array, a map | list, array of the same size, or map whose items (keys and values) fit     |

A schema field typed `Null` takes only `Null`: a value there is data the schema has no type for. Anything else raises `ValueError`, one line per offending field, its path written with `.` between fields, `[]` for a list's items and `{}` for a map's values:

```python
pl.DataFrame({
    "id": ["c1"],
    "data": ['{"name": "Cy", "address": {"city": "Oslo", "floor": 3}, "score": "high"}'],
}).write_parquet("chunk_c.parquet")

try:
    normalise_from_parquet(
        "chunk_c.parquet",
        column="data",
        output_path="out_c.parquet",
        typed=True,
        schema=stored,
    )
except ValueError as e:
    print(e)
print(os.path.exists("out_c.parquet"))
```

```text
The inferred schema of 'data' does not fit the target schema (schema_check="compatible"):
  address.floor: a field the target schema lacks (Int64)
  score: String, the target has Int64
False
```

Nothing is written for a file that fails, including its lookup and pruned files.

With `schema_check="strict"` the inferred schema must be the schema, field for field, so a missing field, or a `Null` where the schema has another dtype, is an error too: `chunk_a` fails it. Use it when every file is expected to use the whole schema. With `schema_check="none"` nothing is checked: fields the schema lacks are dropped, and other scalar dtypes are cast (an error if a value can't be).

### Check a schema without writing: `check_schema_compatibility`

A file written in a schema keeps the schema inferred from its rows in its `genson_avro_schema` metadata. `check_schema_compatibility(inferred, target)` gives the same verdict as `schema_check` for any two schemas, as a list of problems, empty when the inferred schema fits:

```python
from polars_genson import (
    avro_to_polars_schema,
    check_schema_compatibility,
    read_parquet_metadata,
)

meta = read_parquet_metadata("out_a.parquet")
inferred_a = pl.Schema(avro_to_polars_schema(meta["genson_avro_schema"]))
print(inferred_a)
print(check_schema_compatibility(inferred_a, stored))
print(check_schema_compatibility(inferred_a, stored, schema_check="strict"))
```

```text
Schema({'name': String, 'address': Struct({'city': String}), 'score': Null})
[]
['address.postcode: missing, and the schema check is strict', 'score: Null, the target has Int64']
```

### What the file holds

The columns have the schema's dtypes, laid out as Polars writes them to Parquet (strings and lists with 64-bit offsets, list items named `element`), so readers that take the Arrow schema from the file, such as pyarrow, see one schema across files.

`schema` takes strings, binaries, booleans, nulls, integers, floats, structs, lists, arrays and maps; other dtypes (dates and times, decimals, categoricals, enums) raise `ValueError`.

## Write JSON strings instead

Without `typed=True`, the output column holds each normalised row as a JSON string. The inferred schema is stored in the file's metadata, so you can decode the strings later:

```python
from polars_genson import avro_to_polars_schema, read_parquet_metadata

normalise_from_parquet(
    "input.parquet", column="data", output_path="strings.parquet", map_threshold=2
)
meta = read_parquet_metadata("strings.parquet")
dtype = pl.Struct(avro_to_polars_schema(meta["genson_avro_schema"]))
decoded = pl.read_parquet("strings.parquet").select(pl.col("data").str.json_decode(dtype))
print(decoded.unnest("data"))
```

```text
shape: (3, 3)
┌───────┬────────────┬────────────────────────────────┐
│ name  ┆ tags       ┆ labels                         │
│ ---   ┆ ---        ┆ ---                            │
│ str   ┆ list[str]  ┆ map[str, str]                  │
╞═══════╪════════════╪════════════════════════════════╡
│ Alice ┆ ["a", "b"] ┆ {"en": "Alice", "fr": "Alice"} │
│ Bob   ┆ null       ┆ {"de": "Bob"}                  │
│ null  ┆ null       ┆ null                           │
└───────┴────────────┴────────────────────────────────┘
```

If the strings were written with `map_encoding="kv"`, pass the same to `avro_to_polars_schema`; the file's `genson_normalise_config` metadata records it.

This gives the same values as the typed output. Typed output skips writing every row as JSON and parsing it again, so it's faster and needs less memory; use strings when you need the JSON itself.

Both output forms store two metadata entries, which `read_parquet_metadata` returns: `genson_avro_schema` (the inferred schema as Avro JSON) and `genson_normalise_config` (the options used).

## Only infer the schema

`infer_from_parquet` returns the inferred schema without writing any data. It takes the same inference options as `df.genson.infer_json_schema`:

```python
import json
from polars_genson import infer_from_parquet

schema = infer_from_parquet("input.parquet", column="data", map_threshold=2)
print(json.dumps(schema, indent=2))
```

```text
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "name": {
      "type": "string"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "labels": {
      "type": "object",
      "additionalProperties": {
        "type": "string"
      }
    }
  },
  "required": [
    "labels",
    "name",
    "tags"
  ],
  "type": "object"
}
```

Pass `output_path` to write the schema to a file instead, and `avro=True` for an Avro schema.

## Write Arrow data with genson's writer: `write_parquet`

`write_parquet` writes any Arrow data with the Parquet writer genson writes its own output with. It takes anything with an `__arrow_c_stream__` method (a `pyarrow.Table` or `RecordBatchReader`, a Polars `DataFrame`, or the stream `normalise_from_parquet(..., output_format="arrow")` returns), without copying its buffers:

```python
import pyarrow as pa
from polars_genson import write_parquet

table = pa.table({
    "id": ["Q1", "Q2"],
    "refs": [[{"hash": "a", "snaks": ["P1", "P2"]}], None],
})
write_parquet(table, "arrow.parquet", row_group_size=1_000_000)
print(pl.read_parquet("arrow.parquet"))
```

```text
shape: (2, 2)
┌─────┬──────────────────────┐
│ id  ┆ refs                 │
│ --- ┆ ---                  │
│ str ┆ list[struct[2]]      │
╞═════╪══════════════════════╡
│ Q1  ┆ [{"a",["P1", "P2"]}] │
│ Q2  ┆ null                 │
└─────┴──────────────────────┘
```

Each row group's leaf columns are encoded in parallel, with the GIL released, so deeply nested tables, whose leaves a writer otherwise encodes one at a time, write several times faster: Wikidata's claims, one row per statement with their references and qualifiers (1.56 million rows), in 2.3 s on 4 threads against pyarrow's 8.5 s, 5% smaller.

The file holds the same data in the same Arrow schema as `pyarrow.parquet` writes it: list and map groups take the specification's names (`element`, `key_value`), and the Arrow schema is stored in the file's metadata. It is compressed with zstd (level 1), and each leaf column is dictionary encoded but one whose first values are mostly distinct.

## See also

- [Factor out repeated embedded data](https://lmmx.github.io/polars-genson/pr-preview/pr-301/guides/extract-invariants/index.md), for JSON that carries the same nested data in many rows.
- [Remove malformed records](https://lmmx.github.io/polars-genson/pr-preview/pr-301/guides/prune/index.md), for JSON with broken records to leave out.
- [Process large inputs](https://lmmx.github.io/polars-genson/pr-preview/pr-301/guides/large-inputs/index.md), for memory use on big files.
