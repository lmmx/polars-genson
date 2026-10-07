# Examples

Standalone scripts using the published `polars-genson`. Run each with `uv run <script>`.

- `json_to_map.py`: JSON objects whose keys are data (labels keyed by language) become
  `pl.Map` columns, where Polars on its own gives a struct with a null-padded field per
  key.
- `standalone_parquet_demo.py`: infer a schema from a Parquet column of JSON strings and
  normalise it to a new Parquet file.
- `main.py`: memory use while normalising a larger input.
- `video/`: a video explaining records, maps, `pl.Map` and how polars-genson infers
  maps, and a karaoke-style teleprompter for recording its voiceover, made with
  [fframes](https://github.com/Scr44gr/fframes-py). Needs Python 3.11+.

  ```
  uv run --group video video/render.py explainer              # video/json_to_map.mp4
  uv run --group video video/render.py teleprompter           # video/teleprompter.mp4
  uv run --group video video/render.py explainer --pace 3     # words per second
  uv run --group video video/render.py teleprompter --preview # PNG frames only
  ```

  The voiceover lines and default pace are in `video/voiceover.py`, and both videos are
  timed from them. `video/media` holds the IBM Plex fonts (SIL Open Font License,
  `OFL-IBM-Plex.txt`) and the official Polars logos from
  [pola-rs/polars-static](https://github.com/pola-rs/polars-static).
