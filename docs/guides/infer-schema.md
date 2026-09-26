# Infer a schema from a JSON column

genson reads every row of a JSON string column and infers one schema that fits them all.
You can get that schema as a Polars schema, as a JSON Schema, or as an Avro schema.

## As a Polars schema

`infer_polars_schema` gives the dtypes that `normalise_json` will produce:

```python exec="on" source="above" result="text" session="infer"
import polars as pl
import polars_genson

df = pl.DataFrame({"json": [
    '{"id": 1, "name": "Ada", "tags": ["x"]}',
    '{"id": 2, "name": "Bo", "email": "bo@example.com"}',
]})
print(df.genson.infer_polars_schema("json"))
```

## As a JSON Schema

`infer_json_schema` returns the schema as a dict. `required` lists the fields present in
every row:

```python exec="on" source="above" result="text" session="infer"
import json

print(json.dumps(df.genson.infer_json_schema("json"), indent=2))
```

Pass `avro=True` for an Avro schema instead:

```python exec="on" source="above" result="text" session="infer"
print(json.dumps(df.genson.infer_json_schema("json", avro=True), indent=2))
```

## One schema per row

With `merge_schemas=False`, `infer_json_schema` returns each row's own schema instead of
one merged schema, which helps when you want to see which rows differ:

```python exec="on" source="above" result="text" session="infer"
for schema in df.genson.infer_json_schema("json", merge_schemas=False):
    print(sorted(schema["properties"]))
```

## When each row is itself a map: `wrap_root`

Sometimes the top level of each row has keys that are data, such as one key per
language. genson never makes the top level of a row a map (`no_root_map=True`), so each
key would become its own field:

```python exec="on" source="above" result="text" session="infer"
labels = pl.DataFrame({"json": ['{"en": "hello"}', '{"fr": "bonjour", "de": "hallo"}']})
print(labels.genson.infer_polars_schema("json", map_threshold=1))
```

`wrap_root` wraps every row under a key first, so the row becomes a field that can be
a map:

```python exec="on" source="above" result="text" session="infer"
print(labels.genson.infer_polars_schema("json", map_threshold=1, wrap_root="labels"))
```

## Rows in other framings

- **Newline-delimited JSON** in one string: pass `ndjson=True`.
- **A top-level array of objects** is read as one object per element
  (`ignore_outer_array=True`, the default).

## See also

- [Type mapping](../concepts/type-mapping.md) for how each kind of JSON value is typed.
- [Maps and records](../concepts/maps-and-records.md) for `map_threshold`,
  `force_field_types` and `unify_maps`.
- [Normalise a JSON column](normalise.md) to get the data in the inferred shape.
