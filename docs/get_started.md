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

Schema inference builds a schema for every row in parallel (on a thread pool the size of your
CPU core count), then merges them into one. By default every row's schema is held in memory until
that single merge, so peak memory grows with the number of rows.

Pass `max_builders` to cap this: rows are processed in chunks of `max_builders`, and each chunk's
schemas are merged before the next chunk starts. For example, with 1,000,000 rows and
`max_builders=1000`, at most 1000 row schemas are held at once.

```python
schema = df.genson.infer_polars_schema("json_data", max_builders=1000)
```

`max_builders` is accepted by `infer_json_schema`, `infer_polars_schema`, `normalise_json`,
`infer_from_parquet` and `normalise_from_parquet`. Smaller values lower peak memory at some cost in
speed.
