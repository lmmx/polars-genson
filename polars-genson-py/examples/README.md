# Examples

Standalone scripts using the published `polars-genson`. Run each with `uv run <script>`.

- `json_to_map.py`: JSON objects whose keys are data (labels keyed by language) become
  `pl.Map` columns, where Polars on its own gives a struct with a null-padded field per
  key.
- `standalone_parquet_demo.py`: infer a schema from a Parquet column of JSON strings and
  normalise it to a new Parquet file.
- `main.py`: memory use while normalising a larger input.
