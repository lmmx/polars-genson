# Examples

Standalone scripts using the published `polars-genson`. Run each with `uv run <script>`.

- `json_to_map.py`: JSON objects whose keys are data (labels keyed by language) become
  `pl.Map` columns, where Polars on its own gives a struct with a null-padded field per
  key.
- `standalone_parquet_demo.py`: infer a schema from a Parquet column of JSON strings and
  normalise it to a new Parquet file.
- `main.py`: memory use while normalising a larger input.
- `video/json_to_map_video.py`: a 50 second video explaining records, maps, `pl.Map`
  and how polars-genson infers maps, made with
  [fframes](https://github.com/Scr44gr/fframes-py) (`uv run --group video
  video/json_to_map_video.py`, or `--preview` for one PNG per scene). Needs Python 3.11+.
  `video/media` holds the IBM Plex fonts (SIL Open Font License, `OFL-IBM-Plex.txt`) and
  the official Polars logos from [pola-rs/polars-static](https://github.com/pola-rs/polars-static).
