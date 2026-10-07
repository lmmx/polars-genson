# Examples

Standalone scripts using the published `polars-genson`. Run each with `uv run <script>`.

- `json_to_map.py`: JSON objects whose keys are data (labels keyed by language) become
  `pl.Map` columns, where Polars on its own gives a struct with a null-padded field per
  key.
- `standalone_parquet_demo.py`: infer a schema from a Parquet column of JSON strings and
  normalise it to a new Parquet file.
- `main.py`: memory use while normalising a larger input.
- `video/`: a video explaining struct and Map columns and how polars-genson infers
  maps from JSON, with a rolling teleprompter for recording its voiceover, made with
  [fframes](https://github.com/Scr44gr/fframes-py). Needs Python 3.11+.

  ```
  uv run --group video video/render.py                      # all outputs, into video/out
  uv run --group video video/render.py explainer --preview  # PNG frames only
  uv run --group video video/render.py timings              # when each line starts
  ```

  `video/script.md` is the script and its timing in one table: edit a row's seconds or
  add pauses there, and every output follows. `video/media` holds the IBM Plex fonts
  (SIL Open Font License, `OFL-IBM-Plex.txt`) and the official Polars logos from
  [pola-rs/polars-static](https://github.com/pola-rs/polars-static).
