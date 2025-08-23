# Getting Started

## Installation

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

## Install (dev)

```bash
cd polars-genson-py/
uv venv && source .venv/bin/activate
uv sync
```
