# Map types: Avro, Polars and Parquet

Once genson has decided that an object is a map, with keys that are data rather than
field names (see [Maps and records](maps-and-records.md)), that map passes through
several formats on its way to a column. This page follows one map through each of them.

| Where                     | A map of language to label                 |
|---------------------------|--------------------------------------------|
| JSON input                | `{"en": "Hi", "es": "Hola"}`               |
| The inferred Avro schema  | `{"type": "map", "values": "string"}`      |
| A Polars column           | `pl.Map(pl.String, pl.String)`             |
| A typed Parquet file      | a Parquet `MAP`, Arrow `map<string, string>` |
| The `kv` encoding         | `pl.List(pl.Struct({"key": pl.String, "value": pl.String}))` |

## In the schema: an Avro map

genson records its decision in the schema it infers. In Avro, a map is a type of its
own, with one type for all its values:

```python exec="on" source="above" result="text" session="map-types"
import json
import polars as pl
import polars_genson

df = pl.DataFrame({"j": [
    '{"id": 1, "labels": {"en": "Hi", "es": "Hola"}}',
    '{"id": 2, "labels": {"fr": "Bonjour"}}',
]})
schema = df.genson.infer_json_schema("j", map_threshold=1, avro=True)
print(json.dumps(schema["fields"][1], indent=2))
```

Avro map keys are always strings, so every map genson infers has string keys.

## In Polars: `pl.Map`

`normalise_json` and `infer_polars_schema` give a map as a `pl.Map`. Each row holds its
own keys, and only those:

```python exec="on" source="above" result="text" session="map-types"
out = df.genson.normalise_json("j", map_threshold=1)
print(out)
```

Polars' `map` namespace reads it, for example `map.get` for one key, `map.keys` for the
keys, and `map.entries` for the key/value pairs:

```python exec="on" source="above" result="text" session="map-types"
labels = pl.col("labels")
print(out.select(labels.map.get("en").alias("en"), labels.map.keys().alias("keys")))
```

## As JSON text: `map_encoding`

With `decode=False`, `normalise_json` writes each row as a JSON string, and
`map_encoding` chooses how a map is written:

```python exec="on" source="above" result="text" session="map-types"
for encoding in ("mapping", "kv", "entries"):
    text = df.genson.normalise_json(
        "j", map_threshold=1, map_encoding=encoding, decode=False
    )
    print(f"{encoding:8s}", text[0])
```

- `"mapping"` (the default) writes a JSON object, which `str.json_decode` reads as a
  `pl.Map`.
- `"kv"` writes a list of `{"key": ..., "value": ...}` objects, which it reads as a list
  of `{key, value}` structs (see [below](#the-kv-encoding)).
- `"entries"` writes a list of one-key objects. No Polars dtype reads it, so it can't be
  decoded.

Pass the same `map_encoding` to `infer_polars_schema` or `avro_to_polars_schema` and the
schema you get decodes the text:

```python exec="on" source="above" result="text" session="map-types"
text = df.genson.normalise_json("j", map_threshold=1, map_encoding="kv", decode=False)
dtype = pl.Struct(df.genson.infer_polars_schema("j", map_threshold=1, map_encoding="kv"))
print(text.str.json_decode(dtype).struct.unnest())
```

## In Parquet: a `MAP`

`normalise_from_parquet(typed=True)` writes the normalised rows as typed columns, and a
map as a Parquet `MAP`. Polars reads it back as a `pl.Map`:

```python exec="on" source="above" result="text" session="map-types"
from polars_genson import normalise_from_parquet

df.write_parquet("labels.parquet")
normalise_from_parquet(
    "labels.parquet", column="j", output_path="typed.parquet", map_threshold=1, typed=True
)
print(pl.read_parquet_schema("typed.parquet")["j"])
```

Other Parquet readers see a map too: pyarrow reads `labels` as `map<string, string>`.

The file's metadata holds the Avro schema and the options it was written with, including
`map_encoding`, so its dtype can be rebuilt from the file alone:

```python exec="on" source="above" result="text" session="map-types"
from polars_genson import avro_to_polars_schema, read_parquet_metadata

meta = read_parquet_metadata("typed.parquet")
map_encoding = json.loads(meta["genson_normalise_config"])["map_encoding"]
print(avro_to_polars_schema(meta["genson_avro_schema"], map_encoding=map_encoding))
```

## The `kv` encoding

A map can also be held as a list of `{key, value}` structs. Arrow and Parquet store a
map this way underneath, and `map.entries` gives it from a `pl.Map`. A cast goes either
way:

```python exec="on" source="above" result="text" session="map-types"
kv = pl.List(pl.Struct({"key": pl.String, "value": pl.String}))
print(out["labels"].cast(kv).to_list())
```

`map_encoding="kv"` gives this form everywhere: in the JSON text, the dtypes from
`infer_polars_schema`, `avro_to_polars_schema` and `normalise_json`, and the typed
Parquet columns. Use it when your code works with maps as lists, for example to
`explode` a map into one row per key.

When you pass your own schema as `normalise_json(decode=...)`, its maps decide the
encoding: a `pl.Map` is decoded from JSON objects and a list of `{key, value}` structs
from `kv` lists, unless you pass `map_encoding` yourself.

## Limits

- `schema_to_json` writes a `pl.Map` as a list of `{key, value}` structs, the Arrow
  storage, because the Rust polars crate has no map dtype. `json_to_schema` reads it
  back as that list.
- A map's keys are always strings, as in Avro.
