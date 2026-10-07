"""Render the json_to_map example as a video with fframes.

    uv run --group video video/json_to_map_video.py            # writes video/json_to_map.mp4
    uv run --group video video/json_to_map_video.py --preview  # a few PNG frames only

The tables and values on screen are computed with polars-genson when the script runs.
Text is set in DejaVu Sans Mono and DejaVu Sans, which must be installed
(`fc-list | grep DejaVu`).
"""

import argparse
import io
import re
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401  (registers the .genson namespace)
from fframes.compose import (
    Composition,
    Position,
    Rectangle,
    RenderOptions,
    Text,
    TextRun,
    Tween,
    Video,
)

HERE = Path(__file__).parent
W, H, FPS = 1920, 1080, 30
MONO, SANS = "DejaVu Sans Mono", "DejaVu Sans"
BG, FG, DIM = "#0F1117", "#E6E6E6", "#7A808C"
CODE, STRING, NULL, ACCENT = "#6CB6FF", "#8DDB8C", "#E5534B", "#FFD43B"
X0, SIZE, LEADING = 120, 28, 40

# Wikidata-style entities: labels keyed by language, and the languages vary
ROWS = [
    '{"id": "Q64", "labels": {"en": "Berlin", "de": "Berlin", "pl": "Berlin"}}',
    '{"id": "Q90", "labels": {"en": "Paris", "es": "París", "it": "Parigi"}}',
    '{"id": "Q220", "labels": {"en": "Rome", "it": "Roma", "fr": "Rome"}}',
    '{"id": "Q1492", "labels": {"en": "Barcelona", "ca": "Barcelona", "es": "Barcelona"}}',
]

# Highlighted tokens: nulls, the map dtype, and quoted strings
TOKENS = re.compile(r"""(null|Map\(String, String\)|map\[str, str\]|"[^"]*"|'[^']*')""")


def compute():
    """The struct Polars gives on its own, genson's map rows, and map namespace output."""
    plain = pl.read_ndjson(io.StringIO("\n".join(ROWS)))
    entities = pl.DataFrame({"json": ROWS}).genson.normalise_json(
        "json", force_field_types={"labels": "map"}
    )
    labels = pl.col("labels")
    ops = entities.select(
        "id",
        labels.map.get("en").alias("en"),
        labels.map.len().alias("languages"),
        labels.map.contains_key("de").alias("has_de"),
    )
    with pl.Config(tbl_hide_dataframe_shape=True, fmt_str_lengths=40):
        struct_table = str(plain["labels"].struct.unnest()).splitlines()
        ops_table = str(ops).splitlines()
    map_rows = [str(entities.schema)]
    map_rows += [str(row) for row in entities.iter_rows(named=True)]
    return struct_table, map_rows, ops_table


def line(text, y, *, size=SIZE, family=MONO, fill=FG, x=X0, middle=False):
    """One line of text at baseline `y`, with its tokens highlighted.

    Spaces become no-break spaces so runs of them (table padding) are kept.
    """
    runs = []
    for part in TOKENS.split(text):
        if not part:
            continue
        if part == "null":
            color = NULL
        elif part.startswith(("Map(", "map[")):
            color = ACCENT
        elif part[0] in "\"'":
            color = STRING
        else:
            color = fill
        runs.append(TextRun(content=part.replace(" ", "\u00a0"), fill=color))
    return Text(
        content=tuple(runs),
        font_size=size,
        font_family=family,
        fill=fill,
        anchor="baseline",
        text_anchor="middle" if middle else "start",
        position=Position(x=W / 2 if middle else x, y=y),
    )


def appear(item, at):
    """`item` fading in at local time `at`, held to the end of its scene."""
    fade = Tween(from_value=0, to_value=1, duration=0.4, easing="ease_out")
    return Composition(size=(W, H), children=(item,), opacity=fade).at(at)


def scene(start, duration, heading, code=(), lines=(), *, lines_at=1.2, stagger=0.1, note=None):
    """A heading, code lines and output lines, appearing in turn."""
    children = [appear(line(heading, 140, size=40, family=SANS), 0)]
    y = 230
    for text in code:
        children.append(appear(line(text, y, fill=CODE), 0.5))
        y += LEADING
    if note:
        children.append(appear(line(note, y, size=22, family=SANS, fill=DIM), 0.8))
        y += LEADING
    y += LEADING // 2
    for i, text in enumerate(lines):
        children.append(appear(line(text, y + i * LEADING), lines_at + i * stagger))
    return Composition(size=(W, H), children=tuple(children)).at(start, duration=duration)


def build():
    """The whole video, about 32 seconds."""
    struct_table, map_rows, ops_table = compute()
    title = Composition(
        size=(W, H),
        children=(
            appear(line("JSON → pl.Map", 500, size=96, family=SANS, middle=True), 0),
            appear(
                line("polars-genson 1.0 on Polars 2", 590, size=36, family=SANS, fill=DIM, middle=True),
                0.6,
            ),
        ),
    ).at(0, duration=3.5)
    scenes = (
        title,
        scene(
            3.5, 4.5, "Labels keyed by language: the keys are data",
            code=("rows = [",), lines=[*ROWS, "]"], lines_at=0.8, stagger=0.4,
        ),
        scene(
            8, 7, "Polars alone: a column per language, null where a row lacks it",
            code=('pl.read_ndjson(rows)["labels"].struct.unnest()',), lines=struct_table,
        ),
        scene(
            15, 7.5, "polars-genson: one Map column, each row holding its own keys",
            code=('df.genson.normalise_json("json", force_field_types={"labels": "map"})',),
            note="Only 4 rows, so labels is named as a map (genson infers one above 20 distinct keys)",
            lines=map_rows, lines_at=1.4, stagger=0.5,
        ),
        scene(
            22.5, 5.5, "Polars' map namespace works on it",
            code=('labels.map.get("en"), labels.map.len(), labels.map.contains_key("de")',),
            lines=ops_table,
        ),
        Composition(
            size=(W, H),
            children=(
                appear(line("pip install polars-genson[polars]", 470, size=48, fill=ACCENT, middle=True), 0),
                appear(line("polars-genson.vercel.app/concepts/map-types", 560, size=32, family=SANS, fill=FG, middle=True), 0.5),
                appear(line("github.com/lmmx/polars-genson", 620, size=32, family=SANS, fill=DIM, middle=True), 0.8),
            ),
        ).at(28, duration=4),
    )
    background = Rectangle(size=(W, H), fill=BG)
    return Video(
        composition=Composition(duration=32, children=(background, *scenes)),
        resolution=(W, H),
        fps=FPS,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--preview", action="store_true", help="write PNG frames only")
    args = parser.parse_args()
    compiled = build().compile()
    if args.preview:
        out = HERE / "preview"
        out.mkdir(exist_ok=True)
        for seconds in (2, 7, 12, 19, 26, 30):
            path = out / f"frame_{seconds:02d}s.png"
            compiled.save_png(str(path), index=seconds * FPS)
            print(path)
    else:
        path = HERE / "json_to_map.mp4"
        compiled.render(str(path), options=RenderOptions(bitrate=8_000_000))
        print(path)


if __name__ == "__main__":
    main()
