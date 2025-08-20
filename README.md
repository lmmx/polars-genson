# Polars Genson

[![PyPI](https://img.shields.io/pypi/v/polars-genson?color=%2300dc00)](https://pypi.org/project/polars-genson)
[![crates.io: genson-core](https://img.shields.io/crates/v/genson-core.svg?label=genson-core)](https://crates.io/crates/genson-core)
[![crates.io: genson-cli](https://img.shields.io/crates/v/genson-cli.svg?label=genson-cli)](https://crates.io/crates/genson-cli)
[![crates.io: polars-jsonschema-bridge](https://img.shields.io/crates/v/polars-jsonschema-bridge.svg?label=polars-jsonschema-bridge)](https://crates.io/crates/polars-jsonschema-bridge)
[![MIT licensed](https://img.shields.io/crates/l/genson-core.svg)](./LICENSE)

Fast JSON schema inference with support for Polars DataFrames.

## Project Structure

This workspace contains multiple interconnected crates, most people will probably want the Python
package:

### Python Package
- **[polars-genson-py/](polars-genson-py/)** - Python bindings and Polars plugin

#### Quick Start

```bash
pip install polars-genson[polars]
```

```python
import polars as pl
import polars_genson

df = pl.DataFrame({
    "json_data": [
        '{"name": "Alice", "age": 30, "scores": [95, 87]}',
        '{"name": "Bob", "age": 25, "city": "NYC", "active": true}'
    ]
})

json_schema = df.genson.infer_json_schema("json_data")
polars_schema = df.genson.infer_polars_schema("json_data")
```

### Rust Libraries

- **[genson-core/](genson-core/)** - Core JSON schema inference library
- **[polars-jsonschema-bridge/](polars-jsonschema-bridge/)** - JSON Schema ↔ Polars type conversion
- **[genson-cli/](genson-cli/)** - Command-line schema inference tool

## Features

- **Fast schema inference** from JSON strings in Polars columns
- **Dual output formats**: JSON Schema and native Polars schemas
- **Complex type support**: nested objects, arrays, mixed types
- **Multiple JSON formats**: single objects, arrays, NDJSON
- **Rust performance** with Python convenience

## Documentation

Each component has detailed documentation:

- **Python users**: See [polars-genson-py/README.md](polars-genson-py/README.md)
- **Rust developers**: See individual crate READMEs
- **Development**: See [DEVELOPMENT.md](DEVELOPMENT.md)

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
