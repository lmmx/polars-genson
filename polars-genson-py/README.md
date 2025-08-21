# Polars Genson

[![PyPI](https://img.shields.io/pypi/v/polars-genson?color=%2300dc00)](https://pypi.org/project/polars-genson)
[![crates.io: genson-core](https://img.shields.io/crates/v/genson-core.svg?label=genson-core)](https://crates.io/crates/genson-core)
[![crates.io: polars-jsonschema-bridge](https://img.shields.io/crates/v/polars-jsonschema-bridge.svg?label=polars-jsonschema-bridge)](https://crates.io/crates/polars-jsonschema-bridge)
[![Supported Python versions](https://img.shields.io/pypi/pyversions/polars-genson.svg)](https://pypi.org/project/polars-genson)
[![pre-commit.ci status](https://results.pre-commit.ci/badge/github/lmmx/polars-genson/master.svg)](https://results.pre-commit.ci/latest/github/lmmx/polars-genson/master)

A comprehensive Polars plugin for working with JSON schemas. Infer schemas from JSON data and convert between JSON Schema and Polars schema formats with full round-trip support.

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

## Core Workflows

The plugin supports three main workflows:

```python
import polars as pl
import polars_genson

# 1. INFERENCE: JSON data → schemas
df_with_json = pl.DataFrame({"json_col": ['{"name": "Alice", "age": 30}']})
json_schema = df_with_json.genson.infer_json_schema("json_col")
polars_schema = df_with_json.genson.infer_polars_schema("json_col")

# 2. CONVERSION: Polars schema → JSON Schema
df_typed = pl.DataFrame({"name": ["Alice"], "age": [30]})
json_schema = df_typed.genson.serialize_schema_to_json()

# 3. CONVERSION: JSON Schema → Polars schema  
json_schema = {"type": "object", "properties": {"name": {"type": "string"}}}
polars_schema = pl.genson.deserialize_json_schema(json_schema)
```

## Quick Start

### Schema Inference from Data

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

# Infer JSON schema from the data
json_schema = df.genson.infer_json_schema("json_data")
print(json.dumps(json_schema, indent=2))

# Infer Polars schema from the data
polars_schema = df.genson.infer_polars_schema("json_data")
print(polars_schema)
```

### Schema Conversion

```python
# Convert existing DataFrame schema to JSON Schema
df_typed = pl.DataFrame({
    "user_id": [1, 2, 3],
    "name": ["Alice", "Bob", "Charlie"],
    "scores": [[95, 87], [82, 91], [78, 85]],
    "active": [True, False, True]
})

# Serialize Polars schema to JSON Schema
json_schema = df_typed.genson.serialize_schema_to_json(
    title="User Data Schema",
    description="Schema for user information",
    optional_fields=["scores"]
)

print("DataFrame schema as JSON Schema:")
print(json.dumps(json_schema, indent=2))
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "User Data Schema", 
  "description": "Schema for user information",
  "type": "object",
  "properties": {
    "user_id": {"type": "integer"},
    "name": {"type": "string"},
    "scores": {
      "type": "array",
      "items": {"type": "integer"}
    },
    "active": {"type": "boolean"}
  },
  "required": ["active", "name", "user_id"],
  "additionalProperties": false
}
```

```python
# Deserialize JSON Schema back to Polars schema
reconstructed_schema = pl.genson.deserialize_json_schema(json_schema)
print("Reconstructed Polars schema:")
print(reconstructed_schema)
```

```python
Schema({
    'user_id': Int64,
    'name': String, 
    'scores': List(Int64),
    'active': Boolean
})
```

## Method Reference

The `genson` namespace provides methods for both inference and conversion:

### Schema Inference Methods

#### `infer_json_schema(column, **kwargs) -> dict`
Infer JSON schema from JSON data in a column.

**Parameters:**
- `column`: Name of the column containing JSON strings
- `ignore_outer_array`: Whether to treat top-level arrays as streams of objects (default: `True`)
- `ndjson`: Whether to treat input as newline-delimited JSON (default: `False`)
- `merge_schemas`: Whether to merge schemas from all rows (default: `True`)
- `debug`: Whether to print debug information (default: `False`)

#### `infer_polars_schema(column, **kwargs) -> pl.Schema`
Infer Polars schema from JSON data in a column.

**Parameters:**
- `column`: Name of the column containing JSON strings  
- `ignore_outer_array`: Whether to treat top-level arrays as streams of objects (default: `True`)
- `ndjson`: Whether to treat input as newline-delimited JSON (default: `False`)
- `debug`: Whether to print debug information (default: `False`)

### Schema Conversion Methods

#### `serialize_schema_to_json(**kwargs) -> dict`
Convert the DataFrame's schema to JSON Schema format.

**Parameters:**
- `schema_uri`: Schema URI to use (default: JSON Schema 2020-12)
- `title`: Title for the JSON Schema
- `description`: Description for the JSON Schema
- `optional_fields`: List of field names that should be optional
- `additional_properties`: Whether to allow additional properties (default: `False`)
- `debug`: Whether to print debug information (default: `False`)

#### `pl.genson.deserialize_json_schema(json_schema, **kwargs) -> pl.Schema`
Convert a JSON Schema to equivalent Polars schema.

**Parameters:**
- `json_schema`: JSON Schema as dict or string
- `debug`: Whether to print debug information (default: `False`)

## Round-trip Example

Demonstrate full round-trip conversion:

```python
# Start with a complex DataFrame
original_df = pl.DataFrame({
    "user": pl.struct([
        pl.field("name", pl.String),
        pl.field("profile", pl.struct([
            pl.field("age", pl.Int32),
            pl.field("preferences", pl.struct([
                pl.field("theme", pl.String)
            ]))
        ]))
    ]),
    "posts": [
        [{"title": "Hello", "likes": 5}],
        [{"title": "World", "likes": 3}, {"title": "Test", "likes": 1}]
    ]
})

print("Original schema:")
print(original_df.schema)

# Convert to JSON Schema
json_schema = original_df.genson.serialize_schema_to_json(
    title="Complex User Schema"
)

# Convert back to Polars schema
reconstructed_schema = pl.genson.deserialize_json_schema(json_schema)

print("Reconstructed schema:")
print(reconstructed_schema)

# Verify they match (handling type equivalences)
assert len(original_df.schema) == len(reconstructed_schema)
```

## Advanced Usage

### Custom Schema Options

```python
# Fine-tune JSON Schema generation
json_schema = df.genson.serialize_schema_to_json(
    schema_uri="https://mycompany.com/schemas/v1",
    title="Production Data Schema",
    description="Validated schema for production data pipeline",
    optional_fields=["metadata", "debug_info"],
    additional_properties=True  # Allow extra fields
)

# Control inference behavior  
inferred_schema = df.genson.infer_json_schema(
    "json_data",
    ignore_outer_array=False,  # Treat arrays as arrays, not streams
    ndjson=True,              # Handle newline-delimited JSON
    merge_schemas=False       # Get individual schemas per row
)
```

### Working with Complex Types

The plugin handles sophisticated type mappings:

```python
# Complex nested structures
complex_df = pl.DataFrame({
    "metadata": pl.struct([
        pl.field("tags", pl.List(pl.String)),
        pl.field("coordinates", pl.Array(pl.Float64, 2)),
        pl.field("timestamps", pl.List(pl.Datetime)),
        pl.field("categories", pl.Categorical)
    ])
})

# All types convert cleanly to JSON Schema
json_schema = complex_df.genson.serialize_schema_to_json()
```

**Supported type mappings:**
- `String` ↔ `{"type": "string"}`
- `Int64` ↔ `{"type": "integer"}`  
- `Float64` ↔ `{"type": "number"}`
- `Boolean` ↔ `{"type": "boolean"}`
- `List(T)` ↔ `{"type": "array", "items": T}`
- `Array(T, n)` ↔ `{"type": "array", "items": T, "minItems": n, "maxItems": n}`
- `Struct` ↔ `{"type": "object", "properties": {...}}`
- `Datetime` ↔ `{"type": "string", "format": "date-time"}`
- `Date` ↔ `{"type": "string", "format": "date"}`
- `Categorical` ↔ `{"type": "string", "description": "Categorical data"}`

## Use Cases

### Data Pipeline Validation
```python
# Validate incoming data against expected schema
expected_schema = {...}  # Your JSON Schema
incoming_schema = df.genson.infer_json_schema("raw_data")

# Compare schemas, validate compatibility
if schemas_compatible(expected_schema, incoming_schema):
    polars_schema = pl.genson.deserialize_json_schema(expected_schema)
    validated_df = df.cast(polars_schema)
```

### API Documentation
```python
# Generate API documentation from DataFrame schemas
api_schema = response_df.genson.serialize_schema_to_json(
    title="API Response Schema",
    description="Schema for /users endpoint response"
)
# Export to OpenAPI, JSON Schema docs, etc.
```

### Schema Evolution
```python
# Track schema changes over time
old_schema = historical_df.genson.serialize_schema_to_json()
new_schema = current_df.genson.serialize_schema_to_json()
# Analyze differences, plan migrations
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
