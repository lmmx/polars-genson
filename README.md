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

```python
import polars as pl
from polars_genson import infer_json_schema

# Example DataFrame with JSON strings
df = pl.DataFrame({
    "json_data": [
        '{"name": "Alice", "age": 30}',
        '{"name": "Bob", "age": 25, "city": "NYC"}',
        '{"name": "Charlie", "age": 35, "email": "charlie@example.com"}'
    ]
})

# Infer schema from the JSON column
schema = df.genson.infer_schema("json_data")
print(schema)

# Use the schema to safely decode JSON with known structure
decoded_df = df.with_columns(
    pl.col("json_data").str.json_decode(schema=schema).alias("parsed_json")
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
