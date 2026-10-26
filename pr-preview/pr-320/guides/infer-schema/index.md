# Infer a schema from a JSON column

genson reads every row of a JSON string column and infers one schema that fits them all. You can get that schema as a Polars schema, as a JSON Schema, or as an Avro schema.

## As a Polars schema

`infer_polars_schema` gives the dtypes that `normalise_json` will produce:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"json": [
    '{"id": 1, "name": "Ada", "tags": ["x"]}',
    '{"id": 2, "name": "Bo", "email": "bo@example.com"}',
]})
print(df.genson.infer_polars_schema("json"))
```

```text
Schema({'id': Int64, 'name': String, 'tags': List(String), 'email': String})
```

## As a JSON Schema

`infer_json_schema` returns the schema as a dict. `required` lists the fields present in every row:

```python
import json

print(json.dumps(df.genson.infer_json_schema("json"), indent=2))
```

```text
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "id": {
      "type": "integer"
    },
    "name": {
      "type": "string"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "email": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "name"
  ],
  "type": "object"
}
```

Pass `avro=True` for an Avro schema instead:

```python
print(json.dumps(df.genson.infer_json_schema("json", avro=True), indent=2))
```

```text
{
  "type": "record",
  "name": "document",
  "namespace": "genson",
  "fields": [
    {
      "name": "id",
      "type": "int"
    },
    {
      "name": "name",
      "type": "string"
    },
    {
      "name": "tags",
      "type": [
        "null",
        {
          "type": "array",
          "items": "string"
        }
      ]
    },
    {
      "name": "email",
      "type": [
        "null",
        "string"
      ]
    }
  ]
}
```

## One schema per row

With `merge_schemas=False`, `infer_json_schema` returns each row's own schema instead of one merged schema, which helps when you want to see which rows differ:

```python
for schema in df.genson.infer_json_schema("json", merge_schemas=False):
    print(sorted(schema["properties"]))
```

```text
['id', 'name', 'tags']
['email', 'id', 'name']
```

## When each row is itself a map: `wrap_root`

Sometimes the top level of each row has keys that are data, such as one key per language. genson never makes the top level of a row a map (`no_root_map=True`), so each key would become its own field:

```python
labels = pl.DataFrame({"json": ['{"en": "hello"}', '{"fr": "bonjour", "de": "hallo"}']})
print(labels.genson.infer_polars_schema("json", map_threshold=1))
```

```text
Schema({'en': String, 'fr': String, 'de': String})
```

`wrap_root` wraps every row under a key first, so the row becomes a field that can be a map:

```python
print(labels.genson.infer_polars_schema("json", map_threshold=1, wrap_root="labels"))
```

```text
Schema({'labels': Map(String, String)})
```

## Rows in other framings

- **Newline-delimited JSON** in one string: pass `ndjson=True`.
- **A top-level array of objects** is read as one object per element (`ignore_outer_array=True`, the default).

## See also

- [Type mapping](https://lmmx.github.io/polars-genson/pr-preview/pr-320/concepts/type-mapping/index.md) for how each kind of JSON value is typed.
- [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-320/concepts/maps-and-records/index.md) for `map_threshold`, `force_field_types` and `unify_maps`.
- [Normalise a JSON column](https://lmmx.github.io/polars-genson/pr-preview/pr-320/guides/normalise/index.md) to get the data in the inferred shape.
