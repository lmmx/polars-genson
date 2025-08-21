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

The `genson` namespace provides two main methods:

### `infer_json_schema(column, **kwargs) -> dict`
Returns a JSON schema (as a Python dict) following the JSON Schema specification.

**Parameters:**
- `column`: Name of the column containing JSON strings
- `ignore_outer_array`: Whether to treat top-level arrays as streams of objects (default: `True`)
- `ndjson`: Whether to treat input as newline-delimited JSON (default: `False`)
- `merge_schemas`: Whether to merge schemas from all rows (default: `True`)
- `debug`: Whether to print debug information (default: `False`)

### `infer_polars_schema(column, **kwargs) -> pl.Schema`
Returns a Polars schema with native data types for direct use with Polars operations.

**Parameters:**
- `column`: Name of the column containing JSON strings  
- `ignore_outer_array`: Whether to treat top-level arrays as streams of objects (default: `True`)
- `ndjson`: Whether to treat input as newline-delimited JSON (default: `False`)
- `debug`: Whether to print debug information (default: `False`)

**Note:** `merge_schemas=False` is not yet supported for Polars schema inference.

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

## Standalone CLI Tool

The project also includes a standalone command-line tool for JSON schema inference:

```bash
cd genson-cli
cargo run -- input.json
```

Or from stdin:
```bash
echo '{"name": "test", "value": 42}' | cargo run
```

## Contributing

This crate is part of the [polars-genson](https://github.com/lmmx/polars-genson) project. See the main repository for
the [contribution](https://github.com/lmmx/polars-genson/blob/master/CONTRIBUTION.md)
and [development](https://github.com/lmmx/polars-genson/blob/master/DEVELOPMENT.md) docs.

## License

MIT License

- Contains vendored and slightly adapted copy of the Apache 2.0 licensed fork of `genson-rs` crate
