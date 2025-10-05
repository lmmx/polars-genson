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

## Troubleshooting

### Memory usage exceeds RAM

If memory usage exceeds RAM, try reducing the max. schema builders.

For example, if you have a DataFrame with 1000 rows, and call `.genson.normalise_json` on it,
you'll by default get 1000 threads that get scheduled on your available cores. This will mean that
at one moment in time you will have 1000 `genson-rs` schema "builders" all storing their built
schemas, before they are all merged in one go. If you limit to 100 builders, they will be merged 100
at a time, which will reduce the peak RSS (RAM use by the process).
