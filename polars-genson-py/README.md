# Polars Genson

[![PyPI](https://img.shields.io/pypi/v/polars-genson?color=%2300dc00)](https://pypi.org/project/polars-genson)
[![crates.io: genson-core](https://img.shields.io/crates/v/genson-core.svg?label=genson-core)](https://crates.io/crates/genson-core)
[![crates.io: polars-jsonschema-bridge](https://img.shields.io/crates/v/polars-jsonschema-bridge.svg?label=polars-jsonschema-bridge)](https://crates.io/crates/polars-jsonschema-bridge)
[![Supported Python versions](https://img.shields.io/pypi/pyversions/polars-genson.svg)](https://pypi.org/project/polars-genson)
[![pre-commit.ci status](https://results.pre-commit.ci/badge/github/lmmx/polars-genson/master.svg)](https://results.pre-commit.ci/latest/github/lmmx/polars-genson/master)

A Polars plugin for working with JSON schemas. Infer schemas from JSON data and convert between JSON Schema and Polars schema formats.

## Installation

```bash
pip install polars-genson[polars]
```

On older CPUs run:

```bash
pip install polars-genson[polars-lts-cpu]
```

## Features

### Schema Inference
- **JSON Schema Inference**: Generate JSON schemas from JSON strings in Polars columns
- **Polars Schema Inference**: Directly infer Polars data types and schemas from JSON data
- **Multiple JSON Objects**: Handle columns with varying JSON schemas across rows
- **Complex Types**: Support for nested objects, arrays, and mixed types
- **Flexible Input**: Support for both single JSON objects and arrays of objects

### Schema Conversion
- **Polars → JSON Schema**: Convert existing DataFrame schemas to JSON Schema format
- **JSON Schema → Polars**: Convert JSON schemas to equivalent Polars schemas  
- **Round-trip Support**: Full bidirectional conversion with validation
- **Schema Manipulation**: Validate, transform, and standardize schemas

## Usage

The plugin adds a `genson` namespace to Polars DataFrames for schema inference and conversion.

```python
import polars as pl
import polars_genson
import json

# Create a DataFrame with JSON strings
df = pl.DataFrame({
    "json_data": [
        '{"name": "Alice", "age": 30, "scores": [95, 87]}',
        '{"name": "Bob", "age": 25, "city": "NYC", "active": true}',
        '{"name": "Charlie", "age": 35, "metadata": {"role": "admin"}}'
    ]
})

print("Input DataFrame:")
print(df)
```

```python
shape: (3, 1)
┌─────────────────────────────────┐
│ json_data                       │
│ ---                             │
│ str                             │
╞═════════════════════════════════╡
│ {"name": "Alice", "age": 30, "… │
│ {"name": "Bob", "age": 25, "ci… │
│ {"name": "Charlie", "age": 35,… │
└─────────────────────────────────┘
```

### JSON Schema Inference

```python
# Infer JSON schema from the JSON column
schema = df.genson.infer_json_schema("json_data")

print("Inferred JSON schema:")
print(json.dumps(schema, indent=2))
```

```json
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "name": {
      "type": "string"
    },
    "age": {
      "type": "integer"
    },
    "scores": {
      "items": {
        "type": "integer"
      },
      "type": "array"
    }
    "city": {
      "type": "string"
    },
    "active": {
      "type": "boolean"
    },
    "metadata": {
      "properties": {
        "role": {
          "type": "string"
        }
      },
      "required": [
        "role"
      ],
      "type": "object"
    },
  },
  "required": [
    "age",
    "name"
  ],
  "type": "object"
}
```

### Polars Schema Inference

Directly infer Polars data types and schemas:

```python
# Infer Polars schema from the JSON column
polars_schema = df.genson.infer_polars_schema("json_data")

print("Inferred Polars schema:")
print(polars_schema)
```

```python
Schema({
    'name': String,
    'age': Int64,
    'scores': List(Int64),
    'city': String,
    'active': Boolean,
    'metadata': Struct({'role': String}),
})
```

The Polars schema inference automatically handles:
- ✅ **Complex nested structures** with proper `Struct` types
- ✅ **Typed arrays** like `List(Int64)`, `List(String)`
- ✅ **Mixed data types** (integers, floats, booleans, strings)
- ✅ **Optional fields** present in some but not all objects
- ✅ **Deep nesting** with multiple levels of structure

### Map vs Record Inference Control

For objects with varying keys, you can control whether they're inferred as Maps (dynamic key-value pairs) or Records (fixed fields) using the `map_threshold` and `map_max_required_keys` parameters:

```python
# Data with different key patterns
df = pl.DataFrame({
    "json_data": [
        '{"user": {"id": 1, "name": "Alice"}, "attributes": {"source": "web", "campaign": "summer"}}',
        '{"user": {"id": 2, "name": "Bob"}, "attributes": {"source": "mobile"}}'
    ]
})

# Default: both user and attributes become Records
schema_default = df.genson.infer_json_schema("json_data")

# Lower thresholds: distinguish structured Records from dynamic Maps
schema_controlled = df.genson.infer_json_schema("json_data", 
    map_threshold=2,           # Objects with ≥2 keys can be Maps
    map_max_required_keys=1    # Maps can have ≤1 required key
)
```

In the controlled example:
- `user` has 2 required keys (`id`, `name`) > 1 → **Record** (structured)
- `attributes` has 1 required key (`source`) ≤ 1 → **Map** (dynamic)

This gives you fine-grained control over how objects with different key stability patterns are classified.

Here's a section to add to the README that demonstrates the unified maps feature with a simple example:

## Schema Unification

For objects with heterogeneous but compatible record structures, `polars-genson` can **unify** them into a single map schema instead of creating separate fixed fields. This is useful for dynamic data where keys represent similar entities with slightly different structures.

### Unifying Compatible Record Types

```python
import polars as pl

# Example: Letter frequency data with vowel/consonant variants
df = pl.DataFrame({
    "json_data": [
        '{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}, "b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}, "c": {"alphabet": 2, "consonant": 1, "frequency": 0.0278}}}'
    ]
})

# Without unification: creates fixed record with separate a, b, c fields
schema_default = df.genson.infer_json_schema("json_data", avro=True, map_threshold=3)

# With unification: creates map with unified record values
schema_unified = df.genson.infer_json_schema("json_data", avro=True, map_threshold=3, unify_maps=True)
```

**Without unification**, you get separate fields:
```json
{
  "letter": {
    "type": "record",
    "fields": [
      {"name": "a", "type": {...}},
      {"name": "b", "type": {...}},
      {"name": "c", "type": {...}}
    ]
  }
}
```

**With unification** (`unify_maps=True`), compatible records are merged:
```json
{
  "letter": {
    "type": "map",
    "values": {
      "type": "record",
      "fields": [
        {"name": "alphabet", "type": "int"},      // shared field (always present)
        {"name": "frequency", "type": "float"},   // shared field (always present)  
        {"name": "vowel", "type": ["null", "int"]},     // optional (vowels only)
        {"name": "consonant", "type": ["null", "int"]}  // optional (consonants only)
      ]
    }
  }
}
```

### Normalization with Unified Schema

```python
# Normalise with unified schema - each key gets the same record structure
normalized = df.genson.normalise_json("json_data", map_threshold=3, unify_maps=True).to_dicts()

print(normalized[0])
```

Output:
```python
{
  'letter': [
    {'key': 'a', 'value': {'alphabet': 0, 'frequency': 0.0817, 'vowel': 0, 'consonant': None}},
    {'key': 'b', 'value': {'alphabet': 1, 'frequency': 0.0150, 'vowel': None, 'consonant': 0}},
    {'key': 'c', 'value': {'alphabet': 2, 'frequency': 0.0278, 'vowel': None, 'consonant': 1}}
  ]
}
```

### When Unification Fails

Records with **conflicting field types** cannot be unified:

```python
df_conflict = pl.DataFrame({
    "json_data": [
        '{"data": {"person1": {"name": "Alice", "age": 30}, "person2": {"name": "Bob", "age": "twenty-five"}}}'
    ]
})

# age field has conflicting types (int vs string) - unification fails
schema = df_conflict.genson.infer_json_schema("json_data", avro=True, map_threshold=1, unify_maps=True)
# Results in separate record fields, not a unified map
```

The `unify_maps` feature enables more flexible schema inference for semi-structured data while maintaining type safety by rejecting incompatible field combinations.

### Root Wrapping (`wrap_root`)

By default, inferred schemas treat each JSON object as the root.  
Sometimes you may want to **wrap the schema in an extra record layer** — for example, to make Avro schemas compatible with systems that require a named top-level record.

You can control this behavior with the `wrap_root` option:

* `wrap_root="true"` → Wraps using the **column name** as the record name
* `wrap_root="<string>"` → Wraps using the given string as the record name
* `wrap_root=None` (default) → No wrapping (root is just `"document"` for Avro)

#### Example: Avro schema with wrap_root

```python
df = pl.DataFrame({
    "json_data": [
        '{"value": "A"}',
        '{"value": "B"}'
    ]
})

schema = df.genson.infer_json_schema("json_data", avro=True, wrap_root="payload")

print(json.dumps(schema, indent=2))
````

```json
{
  "type": "record",
  "name": "document",
  "namespace": "genson",
  "fields": [
    {
      "name": "payload",
      "type": {
        "type": "record",
        "name": "payload",
        "namespace": "genson.document_types",
        "fields": [
          {
            "name": "value",
            "type": "string"
          }
        ]
      }
    }
  ]
}
```

This is especially useful when:

* Exporting Avro to systems that require a **named top-level record**
* Keeping schema names consistent with your **column names** or **domain models**

## Normalisation

In addition to schema inference, `polars-genson` can **normalise JSON columns** so that every row conforms to a single, consistent Avro schema.

This is especially useful for semi-structured data where fields may be missing, empty arrays/maps may need to collapse to `null`, or numeric/boolean values may sometimes be encoded as strings.

### Features

* Converts empty arrays/maps to `null` (default)
* Preserves empties with `empty_as_null=False`
* Ensures missing fields are inserted with `null`
* Supports per-field coercion of numeric/boolean strings via `coerce_strings=True`
* Supports top-level schema evolution with `wrap_root`

### Example: Map Encoding in Polars

By default, Polars cannot store a dynamic JSON object (`{"en":"Hello","fr":"Bonjour"}`)
without exploding it into a struct with fixed fields padded with nulls.  
`polars-genson` solves this by normalising maps to a **list of key/value structs**:

This representation is schema-stable and preserves all map keys without null-padding.
It matches how Arrow/Parquet model Avro `map` types internally.

```python
import polars as pl
import polars_genson

df = pl.DataFrame({
    "json_data": [
        '{"id": 123, "tags": [], "labels": {}, "active": true}',
        '{"id": 456, "tags": ["x","y"], "labels": {"fr":"Bonjour"}, "active": false}',
        '{"id": 789, "labels": {"en": "Hi", "es": "Hola"}}'
    ]
})

print(df.genson.normalise_json("json_data", map_threshold=0))
````

Output:

```text
shape: (3, 4)
┌─────┬────────────┬──────────────────────────────┬────────┐
│ id  ┆ tags       ┆ labels                       ┆ active │
│ --- ┆ ---        ┆ ---                          ┆ ---    │
│ i64 ┆ list[str]  ┆ list[struct[2]]              ┆ bool   │
╞═════╪════════════╪══════════════════════════════╪════════╡
│ 123 ┆ null       ┆ null                         ┆ true   │
│ 456 ┆ ["x", "y"] ┆ [{"fr","Bonjour"}]           ┆ false  │
│ 789 ┆ null       ┆ [{"en","Hi"}, {"es","Hola"}] ┆ null   │
└─────┴────────────┴──────────────────────────────┴────────┘
```

In the example above, `normalise_json` reshaped jagged JSON into a consistent, schema-aligned form:

* **Row 1**

  * `tags` was present but empty (`[]`) → normalised to `null`
    *(this prevents row elimination when exploding the column)*
  * `labels` was present but empty (`{}`) → normalised to `null`
  * `active` stayed `true`

* **Row 2**

  * `tags` had two values (`["x","y"]`) → preserved as a list of strings
  * `labels` had one entry (`{"fr":"Bonjour"}`) → normalised to a list of **one key:value struct**
  * `active` stayed `false`

* **Row 3**

  * `tags` was missing entirely → injected as `null`
  * `labels` had two entries (`{"en":"Hi","es":"Hola"}`) → normalised to a list of **two key:value structs**
  * `active` was missing → injected as `null`

### Example: Empty Arrays

```python
df = pl.DataFrame({"json_data": ['{"labels": []}', '{"labels": {"en": "Hello"}}']})

out = df.genson.normalise_json("json_data")
print(out)
```

Output:

```text
shape: (2, 1)
┌─────────────────────────────┐
│ normalised                  │
│ ---                         │
│ str                         │
╞═════════════════════════════╡
│ {"labels": null}            │
│ {"labels": {"en": "Hello"}} │
└─────────────────────────────┘
```

### Example: Preserving Empty Arrays

```python
out = df.genson.normalise_json("json_data", empty_as_null=False)
print(out)
```

Output:

```text
┌─────────────────────────────┐
│ normalised                  │
╞═════════════════════════════╡
│ {"labels": []}              │
│ {"labels": {"en": "Hello"}} │
└─────────────────────────────┘
```

### Example: String Coercion

```python
df = pl.DataFrame({
    "json_data": [
        '{"id": "42", "active": "true"}',
        '{"id": 7, "active": false}'
    ]
})

# Default: no coercion
print(df.genson.normalise_json("json_data").to_list())
# ['{"id": null, "active": null}', '{"id": 7, "active": false}']

# With coercion
print(df.genson.normalise_json("json_data", coerce_strings=True).to_list())
# ['{"id": 42, "active": true}', '{"id": 7, "active": false}']
```

### Schema-Aware Decoding

The `decode` parameter can be either a **boolean** or a **schema**.

- `decode=True` → Infer a schema automatically, then decode JSON into native Polars types.  
- `decode=False` → Leave values as normalised JSON strings.  
- `decode=pl.Schema | pl.Struct` → Use your own schema for decoding (skip re-inference).

```python
import polars as pl
import polars_genson

df = pl.DataFrame({
    "json_data": [
        '{"id": 1, "active": true}',
        '{"id": 2, "active": false}'
    ]
})

# Explicit schema
schema = pl.Struct({
    "id": pl.Int64,
    "active": pl.Boolean,
})

# Use schema directly for decoding
decoded = df.genson.normalise_json("json_data", decode=schema)
print(decoded)
```

Output:

```
shape: (2, 2)
┌─────┬────────┐
│ id  ┆ active │
│ --- ┆ ---    │
│ i64 ┆ bool   │
╞═════╪════════╡
│ 1   ┆ true   │
│ 2   ┆ false  │
└─────┴────────┘
```

**Note:** Normalisation always aligns rows to a consistent schema internally.
Passing your own schema skips the extra inference step, which can improve performance,
but if your schema doesn’t match what’s in the data, you'll hit a decoding error
(`polars.exceptions.ComputeError` from `.str.json_decode`). That may in fact be desirable to halt on though.

For the best of both worlds, you can run with decode=True once, capture the resulting `.schema`,
and then reuse it in future calls.

## Advanced Usage

### Per-Row Schema Processing

- Only available with JSON schema currently (per-row/unmerged Polars schemas TODO)

```python
# Get individual schemas and process them
df = pl.DataFrame({
    "ABCs": [
        '{"a": 1, "b": 2}',
        '{"a": 1, "c": true}',
    ]
})

# Analyze schema variations
individual_schemas = df.genson.infer_json_schema("ABCs", merge_schemas=False)
```

The result is a list of one schema per row. With `merge_schemas=True` you would
get all 3 keys (a, b, c) in a single schema.

```
[{'$schema': 'http://json-schema.org/schema#',
  'properties': {'a': {'type': 'integer'}, 'b': {'type': 'integer'}},
  'required': ['a', 'b'],
  'type': 'object'},
 {'$schema': 'http://json-schema.org/schema#',
  'properties': {'a': {'type': 'integer'}, 'c': {'type': 'boolean'}},
  'required': ['a', 'c'],
  'type': 'object'}]
```

### JSON Schema Options

```python
# Use the expression directly for more control
result = df.select(
    polars_genson.infer_json_schema(
        pl.col("json_data"),
        merge_schemas=False,  # Get individual schemas instead of merged
    ).alias("individual_schemas")
)

# Or use with different options
schema = df.genson.infer_json_schema(
    "json_data",
    ignore_outer_array=False,  # Treat top-level arrays as arrays
    ndjson=True,               # Handle newline-delimited JSON
    schema_uri="https://json-schema.org/draft/2020-12/schema",  # Specify a schema URI
    merge_schemas=True         # Merge all schemas (default)
)
```

### Polars Schema Options

```python
# Infer Polars schema with options
polars_schema = df.genson.infer_polars_schema(
    "json_data",
    ignore_outer_array=True,  # Treat top-level arrays as streams of objects
    ndjson=False,            # Not newline-delimited JSON
    debug=False              # Disable debug output
)

# Note: merge_schemas=False not yet supported for Polars schemas
```

## Method Reference

The `genson` namespace provides three main methods:

### `infer_json_schema(column, **kwargs) -> dict | list[dict]`

Infers a JSON Schema (or Avro, if requested) from a string column.

**Parameters:**

* `column`: Name of the column containing JSON strings
* `ignore_outer_array`: Treat top-level arrays as streams of objects (default: `True`)
* `ndjson`: Treat input as newline-delimited JSON (default: `False`)
* `schema_uri`: Schema URI to embed in the output (default: `"http://json-schema.org/schema#"`). *Ignored by some consumers when `avro=True`.*
* `merge_schemas`: Merge schemas from all rows (default: `True`). If `False`, returns one schema **per row** as a list.
* `debug`: Print debug information (default: `False`)
* `map_threshold`: Detect maps when object has more than N keys (default: `20`)
* `map_max_required_keys`: Maximum required keys for Map inference (default: `None`). Objects with more required keys will be forced to Record type. If `None`, no gating based on required key count.
* `force_field_types`: Dict of per-field overrides, values must be `"map"` or `"record"`. Example: `{"labels": "map", "claims": "record"}`
* `avro`: Output Avro schema instead of JSON Schema (default: `False`)
* `wrap_root`: Control root wrapping.

  * `True` → wrap using the **column name**
  * `str` → wrap using the given name
  * `None` → no wrapping (default)

**Returns:**

* `dict` when `merge_schemas=True`
* `list[dict]` when `merge_schemas=False`

### `infer_polars_schema(column, **kwargs) -> pl.Schema`

Infers a native Polars schema from a string column.

**Parameters:**

* `column`: Name of the column containing JSON strings
* `ignore_outer_array`: Treat top-level arrays as streams of objects (default: `True`)
* `ndjson`: Treat input as newline-delimited JSON (default: `False`)
* `merge_schemas`: Merge schemas from all rows (default: `True`). *(Currently the only supported mode.)*
* `debug`: Print debug information (default: `False`)
* `map_threshold`: Detect maps when object has more than N keys (default: `20`)
* `map_max_required_keys`: Maximum required keys for Map inference (default: `None`). Objects with more required keys will be forced to Record type. If `None`, no gating based on required key count.
* `force_field_types`: Dict of per-field overrides, values must be `"map"` or `"record"`
* `avro`: Infer using **Avro semantics** (unions, maps, nullability) instead of pure JSON Schema semantics (default: `False`)
* `wrap_root`: Control root wrapping.

  * `True` → wrap using the **column name**
  * `str` → wrap using the given name
  * `None` → no wrapping (default)

**Returns:**

* `pl.Schema`

**Note:** `merge_schemas=False` is **not** supported for Polars schema inference.

### `normalise_json(column, **kwargs) -> pl.DataFrame | pl.Series`

Normalises each JSON string in the column against a single, inferred **Avro** schema. Ensures every row matches the same structure and datatypes.

**Parameters:**

* `column`: Name of the column containing JSON strings
* `decode`: If `True`, decode to native Polars types (default: `True`)
* `unnest`: If `decode=True`, expand the decoded struct into separate columns (default: `True`)
* `ignore_outer_array`: Treat top-level arrays as streams of objects (default: `True`)
* `ndjson`: Treat input as newline-delimited JSON (default: `False`)
* `empty_as_null`: Convert empty arrays/maps to `null` (default: `True`)
* `coerce_strings`: Coerce numeric/boolean strings (e.g. `"42"`, `"true"`) into numbers/booleans where the schema expects them (default: `False`)
* `map_encoding`: Encoding for Avro maps: `"kv"` (default), `"mapping"`, or `"entries"`
* `map_threshold`: Detect maps when object has more than N keys (default: `20`)
* `map_max_required_keys`: Maximum required keys for Map inference (default: `None`). Objects with more required keys will be forced to Record type. If `None`, no gating based on required key count.
* `force_field_types`: Dict of per-field overrides (`"map"`/`"record"`)
* `wrap_root`: Control root wrapping.

  * `True` → wrap using the **column name**
  * `str` → wrap using the given name
  * `None` → no wrapping (default)

**Returns:**

* If `decode=True`:

  * `unnest=True` → **`pl.DataFrame`** with one column per schema field
  * `unnest=False` → **`pl.DataFrame`** with a single **struct** column
* If `decode=False` → **`pl.Series`** of normalised JSON strings

**Example:**

```python
df = pl.DataFrame({"json_data": ['{"labels": []}', '{"labels": {"en": "Hello"}}']})
out = df.genson.normalise_json("json_data")
print(out.to_list())
# ['{"labels": null}', '{"labels": {"en": "Hello"}}']
```

## Examples

### Working with Complex JSON

```python
# Complex nested JSON with arrays of objects
df = pl.DataFrame({
    "complex_json": [
        '{"user": {"profile": {"name": "Alice", "preferences": {"theme": "dark"}}}, "posts": [{"title": "Hello", "likes": 5}]}',
        '{"user": {"profile": {"name": "Bob", "preferences": {"theme": "light"}}}, "posts": [{"title": "World", "likes": 3}, {"title": "Test", "likes": 1}]}'
    ]
})

schema = df.genson.infer_polars_schema("complex_json")
print(schema)
```

```python
Schema({
    'user': Struct({
        'profile': Struct({
            'name': String, 
            'preferences': Struct({'theme': String})
        })
    }),
    'posts': List(Struct({'likes': Int64, 'title': String})),
})
```

### Using Inferred Schema

```python
# You can use the inferred schema for validation or DataFrame operations
inferred_schema = df.genson.infer_polars_schema("json_data")

# Use with other Polars operations
print(f"Schema has {len(inferred_schema)} fields:")
for name, dtype in inferred_schema.items():
    print(f"  {name}: {dtype}")
```

## Contributing

This crate is part of the [polars-genson](https://github.com/lmmx/polars-genson) project. See the main repository for
the [contribution](https://github.com/lmmx/polars-genson/blob/master/CONTRIBUTION.md)
and [development](https://github.com/lmmx/polars-genson/blob/master/DEVELOPMENT.md) docs.

## License

MIT License

- Contains vendored and slightly adapted copy of the Apache 2.0 licensed fork of `genson-rs` crate
