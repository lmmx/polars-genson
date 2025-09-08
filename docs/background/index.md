# Problems with columnar data

If we read JSON into Polars, it might have the wrong schema.

## Columnar dtype does not permit union data types

For example, if we have a union dtype (a concept Polars doesn't support), then we will either
silently lose data or fail to parse completely (usually the latter).

Take this data with a union dtype for the _strs_ column, either a string or a list of strings:

```py
df = pl.DataFrame(
    {
        "json_data": [
            '{"id": 1, "nums": [0], "strs": "a"}',
            '{"id": 2, "nums": [1, 2], "strs": ["b"]}',
            '{"id": 3, "nums": [3, 4, 5], "strs": ["c", "d", "e"]}',
        ]
    }
)
```

While we can store it as a string dtype `json_data` column, a naive implementation of schema
parsing will decide at the first string that we have a string column:

```py
polars_schema = df.genson.infer_polars_schema("json_data")
print(polars_schema)
print("\nSchema details:")
for name, dtype in polars_schema.items():
    print(f"  {name}: {dtype}")

```

Inferred Polars Schema:

```py
Schema({'id': Int64, 'nums': List(Int64), 'strs': String})

Schema details:
  id: Int64
  nums: List(Int64)
  strs: String
```

The JSON schema column is more expressive, and has an `anyOf` concept to display union dtypes,
because a JSON path is not a 'column' and does not have to have a single data type:

```py
json_schema = df.genson.infer_json_schema("json_data")
print(json.dumps(json_schema, indent=2))
```

Inferred JSON Schema:

```json
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "id": {
      "type": "integer"
    },
    "nums": {
      "type": "array",
      "items": {
        "type": "integer"
      }
    },
    "strs": {
      "anyOf": [
        {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        {
          "type": "string"
        }
      ]
    }
  },
  "required": [
    "id",
    "nums",
    "strs"
  ],
  "type": "object"
}
```

If we were to write that out as if it were Python annotations, it'd be

```py
id: int
nums: list[int]
strs: str | list[str]
```
