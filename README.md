# Polars Genson

[![crates.io](https://img.shields.io/crates/v/genson-core.svg)](https://crates.io/crates/genson-core)
[![PyPI](https://img.shields.io/pypi/v/polars-genson.svg)](https://pypi.org/project/polars-genson)
[![Supported Python versions](https://img.shields.io/pypi/pyversions/polars-genson.svg)](https://pypi.org/project/polars-genson)
[![MIT/Apache-2.0 licensed](https://img.shields.io/crates/l/genson-core.svg)](./LICENSE)
[![pre-commit.ci status](https://results.pre-commit.ci/badge/github/lmmx/polars-genson/master.svg)](https://results.pre-commit.ci/latest/github/lmmx/polars-genson/master)

A Polars plugin for JSON schema inference from string columns using genson-rs.

## Installation

```bash
pip install polars-genson[polars]
```

On older CPUs run:

```bash
pip install polars-genson[polars-lts-cpu]
```

## Features

- **Automatic JSON Schema Inference**: Analyze JSON strings in Polars columns to infer their schema
- **Multiple JSON Objects**: Handle columns with varying JSON schemas across rows
- **Flexible Input**: Support for both single JSON objects and arrays of objects
- **Polars Integration**: Native Polars plugin with familiar API

## Usage

The plugin adds a `genson` namespace to Polars DataFrames for JSON schema inference.

## Quick Start

```python
import polars as pl
import polars_genson
import json

# Create a DataFrame with JSON strings
df = pl.DataFrame({
    "json_data": [
        '{"name": "Alice", "age": 30}',
        '{"name": "Bob", "age": 25, "city": "NYC"}',
        '{"name": "Charlie", "age": 35, "email": "charlie@example.com"}'
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
│ {"name": "Alice", "age": 30}    │
│ {"name": "Bob", "age": 25, "ci… │
│ {"name": "Charlie", "age": 35,… │
└─────────────────────────────────┘
```

```python
# Infer schema from the JSON column using the genson namespace
schema = df.genson.infer_json_schema("json_data")

print("Inferred schema:")
print(json.dumps(schema, indent=2))
```

```json
{
  "$schema": "http://json-schema.org/schema#",
  "properties": {
    "age": {
      "type": "integer"
    },
    "city": {
      "type": "string"
    },
    "email": {
      "type": "string"
    },
    "name": {
      "type": "string"
    }
  },
  "required": [
    "age",
    "name"
  ],
  "type": "object"
}
```

The plugin automatically:
- ✅ **Merges schemas** from all JSON objects in the column
- ✅ **Identifies required fields** (present in all objects)
- ✅ **Detects optional fields** (present in some objects)
- ✅ **Infers correct types** (string, integer, etc.)

## Advanced Usage

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
    ndjson=True,              # Handle newline-delimited JSON
    merge_schemas=True        # Merge all schemas (default)
)
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

## Development

To build the project:

1. Build the core library:
   ```bash
   cd genson-core
   cargo build
   ```

2. Build the CLI tool:
   ```bash
   cd genson-cli
   cargo build
   ```

3. Build the Python bindings:
   ```bash
   cd polars-genson-py
   maturin develop
   ```

## License

MIT License
