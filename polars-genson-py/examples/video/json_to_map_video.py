"""The explainer: struct and Map columns, and how polars-genson infers maps from JSON.

Each scene follows a scene of script.md, its visuals cued to the scene's `say` rows.
Every value on screen is computed with Polars and polars-genson when the script runs.
Fonts (IBM Plex, OFL) and the Polars logos are in video/media. Render with render.py.
"""

import io
import math
import random
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401  (registers the .genson namespace)
from fframes.compose import (
    Circle,
    Close,
    Composition,
    Image,
    LineTo,
    MoveTo,
    Pattern,
    Position,
    RadialGradient,
    Rectangle,
    Samples,
    Stop,
    Stroke,
    Text,
    TextRun,
    Tween,
    VectorPath,
    Video,
)
from fontmetrics import width

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"

W, H, FPS = 1920, 1080, 30
X = 160  # left margin

BG, GLOW, PANEL, EDGE = "#0B0F19", "#16264A", "#161C2A", "#2A3550"
FG, DIM, JSON_FG = "#E8ECF4", "#8A93A6", "#C9D1E0"
BLUE, CODE, NULL = "#2F8CFF", "#6CB6FF", "#E5534B"
NULL_FILL = "#B8BEC9"  # behind a null
FIELD = "#3DDC84"  # struct fields
# One pastel per language, in the order of the struct's fields
TINTS = (
    "#4C9AFF",
    "#F2A541",
    "#E86A92",
    "#B48CF2",
    "#4FD1C5",
    "#F6E05E",
    "#FF8A65",
    "#A3B8FF",
)

PEOPLE = [
    '{"name": "Ada Lovelace", "born": "1815-12-10"}',
    '{"name": "Alan Turing", "born": "1912-06-23"}',
]
# Labels from Wikidata, a few languages each
CITIES = [
    '{"id": "Q64", "labels": {"en": "Berlin", "de": "Berlin", "fr": "Berlin", "pl": "Berlin"}}',
    '{"id": "Q84", "labels": {"en": "London", "ca": "Londres", "es": "Londres"}}',
    '{"id": "Q90", "labels": {"en": "Paris", "fr": "Paris", "es": "París", "it": "Parigi", "pt": "Paris"}}',
    '{"id": "Q220", "labels": {"en": "Rome", "it": "Roma"}}',
]
THRESHOLD = 5  # the map_threshold the video lowers to
AXIS_KEYS = 25  # the number line runs from 0 to this many distinct keys
# Map key dtypes, each with an example key
KEY_TYPES = (
    ("String", '"en"'),
    ("Int64", "42"),
    ("Date", "2026-10-07"),
    ("Boolean", "true"),
    ("Categorical", '"red"'),
)


# ---------------------------------------------------------------- data


def compute():
    """Everything the video shows, from Polars and polars-genson."""
    people = pl.read_ndjson(io.StringIO("\n".join(PEOPLE))).with_columns(
        pl.col("born").str.to_date()
    )
    df = pl.DataFrame({"json": CITIES})
    grid = pl.read_ndjson(io.StringIO("\n".join(CITIES)))["labels"].struct.unnest()
    default = df.genson.infer_polars_schema("json")["labels"]
    lowered = df.genson.infer_polars_schema("json", map_threshold=THRESHOLD)["labels"]
    forced = df.genson.infer_polars_schema("json", force_field_types={"labels": "map"})[
        "labels"
    ]
    assert isinstance(default, pl.Struct), default
    assert lowered == forced == pl.Map(pl.String, pl.String), (lowered, forced)
    for dtype, _ in KEY_TYPES:  # each is a valid Map key dtype
        pl.Series([None], dtype=pl.Map(getattr(pl, dtype), pl.String))

    cities = df.genson.normalise_json("json", map_threshold=THRESHOLD)
    labels = pl.col("labels")
    return {
        "people": people,
        "ids": cities["id"].to_list(),
        "langs": grid.columns,
        "grid": grid.rows(),
        "maps": cities["labels"].to_list(),
        "default_fields": len(default.fields),
        "map_dtype": str(lowered),
        "get_en": cities.select(labels.map.get("en")).to_series().to_list(),
        "get_es": cities.select(labels.map.get("es")).to_series().to_list(),
        "len": cities.select(labels.map.len()).to_series().to_list(),
    }


# ---------------------------------------------------------------- drawing


def text(content, x, y, *, size=44, family=SANS, weight=400, fill=FG, align="start"):
    """Text with its baseline at `y`; `content` is a string or (text, colour) pairs.

    Spaces become no-break spaces, so indentation and runs of spaces are kept.
    """
    if isinstance(content, str):
        content = ((content, fill),)
    runs = tuple(
        TextRun(content=part.replace(" ", "\u00a0"), fill=color)
        for part, color in content
        if part
    )
    return Text(
        content=runs,
        font_size=size,
        font_family=family,
        font_weight=weight,
        fill=fill,
        anchor="baseline",
        text_anchor=align,
        position=Position(x=x, y=y),
    )


def mono(content, x, y, size=34, **kwargs):
    return text(content, x, y, size=size, family=MONO, **kwargs)


def mono_width(chars, size):
    """Width of `chars` characters of IBM Plex Mono (600 units per em)."""
    return chars * size * 0.6


def layer(*children, **kwargs):
    return Composition(size=(W, H), children=tuple(children), **kwargs)


def box(x, y, w, h, fill, radius=14, edge=None):
    """A rounded rectangle with its top-left corner at (x, y), and an optional outline."""
    stroke = Stroke(color=edge, width=2) if edge else None
    rect = Rectangle(size=(w, h), radius=radius, fill=fill, stroke=stroke)
    return layer(rect, position=Position(x=x, y=y))


def panel(x, y, w, h, radius=14):
    return box(x, y, w, h, PANEL, radius, edge=EDGE)


def logo(name, x, y, w, h):
    """A Polars logo from video/media."""
    image = Image(source=str(MEDIA / name), size=(w, h), fit="contain")
    return layer(image, position=Position(x=x, y=y))


def enter(items, at, *, dy=30):
    """`items` rising `dy` px and fading in at local time `at`."""
    if not isinstance(items, (list, tuple)):
        items = (items,)
    fade = Tween(from_value=0, to_value=1, duration=0.4, start_at=at, easing="ease_out")
    rise = Tween(
        from_value=dy, to_value=0, duration=0.5, start_at=at, easing="ease_out"
    )
    return layer(*items, opacity=fade, position=Position(x=0, y=rise))


def leave(items, at, duration=0.3):
    """`items` fading out at local time `at`."""
    if not isinstance(items, (list, tuple)):
        items = (items,)
    fade = Tween(
        from_value=1, to_value=0, duration=duration, start_at=at, easing="ease_in"
    )
    return layer(*items, opacity=fade)


def flash(items, at, hold=0.15):
    """`items` appearing at `at` and fading straight away."""
    return leave([enter(items, at, dy=0)], at + hold, duration=0.35)


def scene(s, children, *, fade_in=True, fade_out=True):
    """Scene `s` of the timeline, fading in and out (or cutting, for a seamless join)."""
    inner = layer(*children)
    if fade_out:
        out = Tween(
            from_value=1,
            to_value=0,
            duration=0.3,
            start_at=s.duration - 0.3,
            easing="ease_in",
        )
        inner = layer(inner, opacity=out)
    if fade_in:
        inner = layer(
            inner,
            opacity=Tween(from_value=0, to_value=1, duration=0.4, easing="ease_out"),
        )
    return inner.at(s.start, duration=s.duration)


def heading(words, accent=BLUE):
    """The slide's headline, with a bar in its accent colour."""
    return enter(
        (
            box(X - 34, 128, 10, 54, accent, radius=5),
            text(words, X, 170, size=56, weight=600),
        ),
        0,
    )


def code_card(lines, x, y, *, size=34, fill=CODE, marks=()):
    """Code in a panel, top-left at (x, y), with `marks` ((substring, colour)) picked out."""
    lines = lines.splitlines()
    leading = size * 1.45
    w = mono_width(max(len(line) for line in lines), size) + 56
    h = leading * len(lines) + 32
    items = [panel(x, y, w, h)]
    for i, line in enumerate(lines):
        runs = [(line, fill)]
        for mark, color in marks:
            runs = [
                piece
                for part, c in runs
                for piece in _split_mark(part, c, mark, color, fill)
            ]
        items.append(mono(tuple(runs), x + 28, y + 16 + size + i * leading, size))
    return items


def _split_mark(part, color, mark, mark_color, plain):
    if color != plain or mark not in part:
        return [(part, color)]
    before, _, after = part.partition(mark)
    return [
        (before, color),
        (mark, mark_color),
        *_split_mark(after, color, mark, mark_color, plain),
    ]


def chip(key, value, x, y, tint, size=32, h=60, pad=16):
    """A map entry: the key on its tint, the value beside it. Returns (items, width)."""
    kw = mono_width(len(key), size) + 2 * pad
    vw = mono_width(len(value), size) + 2 * pad
    base = y + h / 2 + size * 0.35
    items = [
        box(x, y, kw + vw, h, PANEL, edge=EDGE),
        box(x, y, kw, h, tint),
        mono(key, x + pad, base, size, weight=600, fill=BG),
        mono(value, x + kw + pad, base, size),
    ]
    return items, kw + vw


def key_chip(key, x, y, tint, size=26, h=52, pad=12):
    """Just a key, on its tint. Returns (items, width)."""
    w = mono_width(len(key), size) + 2 * pad
    return [
        box(x, y, w, h, tint),
        mono(key, x + pad, y + h / 2 + size * 0.35, size, weight=600, fill=BG),
    ], w


def cell(value, x, y, w, h=64, size=30, fill=PANEL, color=FG):
    """A table cell holding `value`; a null is dark text on light grey."""
    if value is None:
        shown, fill, color = "null", NULL_FILL, BG
    else:
        shown = str(value)
    return [
        box(x, y, w, h, fill, radius=10),
        mono(shown, x + 20, y + h / 2 + size * 0.35, size, fill=color),
    ]


def outline(x, y, w, h):
    """A white ring around a chip at (x, y), `w` wide and `h` high."""
    ring = Rectangle(
        size=(w + 12, h + 12), radius=18, fill=None, stroke=Stroke(color=FG, width=3)
    )
    return layer(ring, position=Position(x=x - 6, y=y - 6))


def light_up(line, x, y, size, key, color, at):
    """`key` (a JSON key in `line`, drawn at x, y) turning `color` at time `at`."""
    token, cw, items = f'"{key}":', size * 0.6, []
    start = line.find(token)
    while start != -1:
        items.append(mono(f'"{key}"', x + start * cw, y, size, fill=color))
        start = line.find(token, start + 1)
    return enter(items, at, dy=0)


def data_rows(d, top, step, *, size=26, h=52, pad=12, x0=X + 170):
    """The cities and their labels as chips: (items, {(row, key): (x, y, w)})."""
    tint = dict(zip(d["langs"], TINTS))
    items, places = [], {}
    for i, (id_, labels) in enumerate(zip(d["ids"], d["maps"])):
        y = top + i * step
        items.append(mono(id_, X, y + h / 2 + 11, 28, fill=DIM))
        x = x0
        for key, value in labels.items():
            chip_items, width = chip(
                key, value, x, y, tint[key], size=size, h=h, pad=pad
            )
            items += chip_items
            places[i, key] = (x, y, width)
            x += width + 10
    return items, places


# The struct grid, shared by the struct scene and the start of the map scene
GRID_X, GRID_Y, GRID_W, GRID_H, GRID_GAP = X + 180, 290, 160, 90, 12


def grid_cell_at(i, j):
    """Top-left of the grid cell for city `i`, language `j` (row 0 is the header)."""
    return GRID_X + j * (GRID_W + GRID_GAP), GRID_Y + (i + 1) * (GRID_H + GRID_GAP)


def grid_items(d, *, values=True):
    """The struct grid as (columns, ids): language headers with (with `values`) cells.

    Its text is the size of the map scene's chips, which it becomes.
    """
    tint = dict(zip(d["langs"], TINTS))
    size = 28
    columns = []
    for j, lang in enumerate(d["langs"]):
        x = GRID_X + j * (GRID_W + GRID_GAP)
        label_y = GRID_Y + GRID_H / 2 + size * 0.35
        column = [
            box(x, GRID_Y, GRID_W, GRID_H, tint[lang], radius=10),
            mono(
                lang, x + GRID_W / 2, label_y, size, weight=600, fill=BG, align="middle"
            ),
        ]
        if values:
            for i, row in enumerate(d["grid"]):
                column += cell(
                    row[j], *grid_cell_at(i, j), GRID_W, GRID_H, size, tint[lang] + "33"
                )
        columns.append(column)
    ids = [
        mono(id_, X, grid_cell_at(i, 0)[1] + GRID_H / 2 + 11, 30, fill=DIM)
        for i, id_ in enumerate(d["ids"])
    ]
    return columns, ids


# ---------------------------------------------------------------- scenes


def title_scene(s, d):
    logo_w, gap, version = 379, 28, "2.0"
    total = logo_w + gap + mono_width(len(version), 72)
    x = (W - total) / 2
    return scene(
        s,
        (
            logo("polars_logo_white_text.png", x, 200, logo_w, 90),
            text(version, x + logo_w + gap, 280, size=72, weight=600, fill=BLUE),
            enter(
                text("polars-genson", W / 2, 560, size=140, weight=700, align="middle"),
                0.3,
            ),
            enter(
                text(
                    (("infer ", FG), ("pl.Map", BLUE), (" columns from JSON", FG)),
                    W / 2,
                    680,
                    size=60,
                    align="middle",
                ),
                s.cue(1),
            ),
        ),
        fade_in=False,
    )


def records_scene(s, d):
    people = d["people"]
    size, cw = 44, 44 * 0.6
    items = [heading("Fields: keys that are part of the type", FIELD)]
    width = mono_width(max(map(len, PEOPLE)), size) + 64
    rows = ((PEOPLE[0], 380), (PEOPLE[1], 600))
    for i, (line, y) in enumerate(rows):
        items.append(
            enter(
                (
                    panel(X, y - 66, width, 96),
                    mono(line, X + 32, y, size, fill=JSON_FG),
                ),
                s.cue(0) + i * 0.3,
            )
        )
        for k, field in enumerate(people.columns):
            items.append(
                light_up(line, X + 32, y, size, field, FIELD, s.cue(1) + 0.6 + k * 0.6)
            )
    # Under the first row's values, their types
    line, y = rows[0]
    dtypes = {pl.String: "String", pl.Date: "Date"}
    tags = []
    for field in people.columns:
        value_at = line.index(":", line.index(f'"{field}"')) + 2
        value_end = line.index('"', value_at + 1) + 1
        mid = X + 32 + (value_at + value_end) / 2 * cw
        name = dtypes[people.schema[field]]
        tw = mono_width(len(name), 30) + 32
        tags += [
            box(mid - tw / 2, y + 48, tw, 50, "#24304A", radius=10, edge=CODE + "66"),
            mono(name, mid, y + 83, 30, fill=CODE, align="middle"),
        ]
    items.append(enter(tags, s.cue(4)))
    struct = [("Struct({", FG)]
    for k, field in enumerate(people.columns):
        struct += [
            (f"'{field}'", FIELD),
            (f": {dtypes[people.schema[field]]}", FG),
            (", " if k + 1 < people.width else "", FG),
        ]
    struct.append(("})", FG))
    items.append(enter(mono(tuple(struct), X, 860, 52, weight=600), s.cue(6)))
    return scene(s, items)


def maps_scene(s, d):
    tint = dict(zip(d["langs"], TINTS))
    size = 30
    items = [
        heading("Maps: keys that are data"),
        enter(
            text("Source: Wikidata", W - X, 170, size=30, fill=DIM, align="end"),
            s.cue(1),
        ),
    ]
    lines = []
    for i, (id_, row) in enumerate(zip(d["ids"], CITIES)):
        y = 350 + i * 130
        labels = row[row.index('"labels": ') + len('"labels": ') : -1]
        lines.append((labels, y))
        width = mono_width(len(labels), size) + 48
        items.append(
            enter(
                (
                    mono(id_, X, y, 32, fill=DIM),
                    panel(X + 156, y - 50, width, 74),
                    mono(labels, X + 180, y, size, fill=JSON_FG),
                ),
                s.cue(1) + i * 0.3,
            )
        )
    # Each language lights up in its colour, in every row at once
    for k, lang in enumerate(d["langs"]):
        at = s.cue(2) + 0.4 + k * 0.35
        items += [
            light_up(labels, X + 180, y, size, lang, tint[lang], at)
            for labels, y in lines
        ]
    return scene(s, items)


def struct_scene(s, d):
    nulls = sum(v is None for row in d["grid"] for v in row)
    cells = len(d["grid"]) * len(d["langs"])
    columns, ids = grid_items(d)
    title = heading(f"As a struct: {nulls} of {cells} values are null", FIELD)
    items = [leave([title], s.duration - 0.3), enter(ids, s.cue(0))]
    items += [enter(column, s.cue(0) + j * 0.15) for j, column in enumerate(columns)]
    # No fade out: the map scene starts on this frame
    return scene(s, items, fade_out=False)


def map_scene(s, d):
    """The struct grid becoming map chips, in one shot from the struct scene's last frame.

    The nulls fade; each value slides into its city's row as a chip's value, as a copy
    of its column's language header flies down beside it as the chip's key.
    """
    tint = dict(zip(d["langs"], TINTS))
    fade_nulls, move_at, move = 0.5, 1.1, 1.5
    tween = dict(duration=move, start_at=move_at, easing="ease_in_out")
    size, h, pad, top, step = 28, 64, 16, 300, 106
    cw = size * 0.6
    items = [enter(heading("As a map: no nulls"), 0.1)]

    # The headers stay until their keys have left, and the nulls fade first
    headers, _ = grid_items(d, values=False)
    items.append(
        leave(
            [item for column in headers for item in column], move_at + 0.3, duration=0.6
        )
    )
    nulls = [
        item
        for i, row in enumerate(d["grid"])
        for j, value in enumerate(row)
        if value is None
        for item in cell(None, *grid_cell_at(i, j), GRID_W, GRID_H, size)
    ]
    items.append(leave(nulls, fade_nulls, duration=0.5))

    for i, (id_, labels) in enumerate(zip(d["ids"], d["maps"])):
        row_y = top + i * step
        grid_row_y = grid_cell_at(i, 0)[1]
        # The id moves with its row
        id_y = Tween(
            from_value=grid_row_y + GRID_H / 2 + 11,
            to_value=row_y + h / 2 + 11,
            **tween,
        )
        items.append(
            layer(mono(id_, X, 0, 30, fill=DIM), position=Position(x=0, y=id_y))
        )
        x = X + 180
        for key, value in labels.items():
            j = d["langs"].index(key)
            kw = len(key) * cw + 2 * pad
            vw = len(value) * cw + 2 * pad
            gx, gy = grid_cell_at(i, j)
            # The value: a struct cell shrinking into the chip's value half
            fill_out = Tween(from_value=1, to_value=0, duration=move, start_at=move_at)
            fill_in = Tween(from_value=0, to_value=1, duration=move, start_at=move_at)
            vsize = (
                Tween(from_value=GRID_W, to_value=vw, **tween),
                Tween(from_value=GRID_H, to_value=h, **tween),
            )
            value_text = mono(value, 0, 0, size)
            value_text = layer(
                value_text,
                position=Position(
                    x=Tween(from_value=20, to_value=pad, **tween),
                    y=Tween(
                        from_value=GRID_H / 2 + size * 0.35,
                        to_value=h / 2 + size * 0.35,
                        **tween,
                    ),
                ),
            )
            items.append(
                layer(
                    layer(
                        Rectangle(size=vsize, radius=10, fill=tint[key] + "33"),
                        opacity=fill_out,
                    ),
                    layer(
                        Rectangle(
                            size=vsize,
                            radius=10,
                            fill=PANEL,
                            stroke=Stroke(color=EDGE, width=2),
                        ),
                        opacity=fill_in,
                    ),
                    value_text,
                    position=Position(
                        x=Tween(from_value=gx, to_value=x + kw, **tween),
                        y=Tween(from_value=gy, to_value=row_y, **tween),
                    ),
                )
            )
            # The key: a copy of the language header flying down beside its value
            hx = GRID_X + j * (GRID_W + GRID_GAP)
            ksize = (
                Tween(from_value=GRID_W, to_value=kw, **tween),
                Tween(from_value=GRID_H, to_value=h, **tween),
            )
            key_text = layer(
                mono(key, 0, 0, size, weight=600, fill=BG, align="middle"),
                position=Position(
                    x=Tween(from_value=GRID_W / 2, to_value=kw / 2, **tween),
                    y=Tween(
                        from_value=GRID_H / 2 + size * 0.35,
                        to_value=h / 2 + size * 0.35,
                        **tween,
                    ),
                ),
            )
            items.append(
                layer(
                    Rectangle(size=ksize, radius=10, fill=tint[key]),
                    key_text,
                    position=Position(
                        x=Tween(from_value=hx, to_value=x, **tween),
                        y=Tween(from_value=GRID_Y, to_value=row_y, **tween),
                    ),
                )
            )
            x += kw + vw + 14
    items.append(
        enter(
            (
                logo("polars_logo_blue.png", X, 800, 200, 100),
                mono(
                    (("labels: ", FG), (d["map_dtype"], BLUE)),
                    X + 240,
                    870,
                    52,
                    weight=600,
                ),
            ),
            s.cue(3),
        )
    )
    return scene(s, items, fade_in=False)


def keys_scene(s, d):
    items = [
        heading("Map keys: any row-encodable type"),
        enter(
            mono(
                (("pl.Map(", FG), ("key", BLUE), (", value)", FG)),
                X,
                320,
                56,
                weight=600,
            ),
            s.cue(0),
        ),
    ]
    # A card per key dtype: the dtype, and an example key
    cw, ch, gap, y = 232, 170, 22, 390
    for i, (dtype, example) in enumerate(KEY_TYPES):
        x = X + i * (cw + gap)
        items.append(
            enter(
                (
                    panel(x, y, cw, ch),
                    text(
                        dtype,
                        x + cw / 2,
                        y + 62,
                        size=34,
                        weight=600,
                        fill=BLUE,
                        align="middle",
                    ),
                    mono(
                        example, x + cw / 2, y + 130, 30, fill=JSON_FG, align="middle"
                    ),
                ),
                s.cue(1) + i * 0.25,
            )
        )
    x = X + len(KEY_TYPES) * (cw + gap)
    items.append(
        enter(
            (
                box(x, y, cw, ch, "#2A1418", edge=NULL),
                text(
                    "Object",
                    x + cw / 2,
                    y + 62,
                    size=34,
                    weight=600,
                    fill=NULL,
                    align="middle",
                ),
                box(x + 30, y + 50, cw - 60, 4, NULL, radius=2),
                text(
                    "not row-encodable",
                    x + cw / 2,
                    y + 128,
                    size=24,
                    fill=NULL,
                    align="middle",
                ),
            ),
            s.cue(2),
        )
    )
    genson = (
        ("polars-genson:  ", DIM),
        ("pl.Map(", FG),
        ("String", BLUE),
        (", value)", FG),
    )
    items.append(enter(mono(genson, X, 720, 56, weight=600), s.cue(3)))
    return scene(s, items)


def naming_scene(s, d):
    call = (
        'df.genson.normalise_json(\n    "json", force_field_types={"labels": "map"}\n)'
    )
    size = 44
    width = mono_width(max(map(len, call.splitlines())), size) + 56
    marks = (("force_field_types", FIELD), ('"map"', BLUE))
    return scene(
        s,
        (
            heading("Choose which fields become maps"),
            enter(
                code_card(call, (W - width) / 2, 330, size=size, marks=marks), s.cue(1)
            ),
            enter(
                mono(
                    (("labels: ", FG), (d["map_dtype"], BLUE)),
                    W / 2,
                    760,
                    64,
                    weight=600,
                    align="middle",
                ),
                s.cue(2) + 0.8,
            ),
        ),
    )


def inferring_scene(s, d):
    tint = dict(zip(d["langs"], TINTS))
    glide = s.cue(5)  # "Lower it to 5, and they become a map."
    n = len(d["langs"])
    items = [heading("Or let polars-genson decide")]

    # The data, then each distinct key flying from its first appearance into a counter
    rows, places = data_rows(d, 230, 64, size=24, h=46)
    items.append(enter(rows, s.cue(0)))
    cx, cy = 1580, 330  # where the keys land: the counter
    items.append(
        enter(
            text("distinct keys", cx, 290, size=30, fill=DIM, align="middle"), s.cue(2)
        )
    )
    first = {}
    for (_, key), place in places.items():
        first.setdefault(key, place)
    counting, flight, stagger = (
        s.cue(2) + 0.2,
        1.4,
        0.45,
    )  # several keys in flight at once
    arrivals = []
    for k, key in enumerate(first):
        x, y, _ = first[key]
        t = counting + k * stagger
        chip_items, w = key_chip(key, 0, 0, tint[key], size=24, h=46)
        move = dict(duration=flight, start_at=t, easing="ease_in_out")
        flying = layer(
            *chip_items,
            position=Position(
                x=Tween(from_value=x, to_value=cx - w / 2, **move),
                y=Tween(from_value=y, to_value=cy, **move),
            ),
            opacity=Tween(from_value=0, to_value=1, duration=0.1, start_at=t),
        )
        items.append(leave([flying], t + flight))
        arrivals.append(t + flight)
    shown = [s.cue(2), *arrivals, s.duration]
    for count in range(n + 1):
        number = text(str(count), cx, 440, size=120, weight=700, align="middle")
        items.append(
            layer(number).at(shown[count], duration=shown[count + 1] - shown[count])
        )

    # Distinct keys on a line: struct up to map_threshold, map beyond it
    def px(keys):
        return X + keys * (W - 2 * X) / AXIS_KEYS

    band, band_h, end = 580, 70, px(AXIS_KEYS)
    move = dict(duration=1.4, start_at=glide, easing="ease_in_out")
    struct_w = Tween(from_value=px(20) - X, to_value=px(THRESHOLD) - X, **move)
    map_x = Tween(from_value=px(20), to_value=px(THRESHOLD), **move)
    map_w = Tween(from_value=end - px(20), to_value=end - px(THRESHOLD), **move)
    line = [
        layer(
            Rectangle(size=(struct_w, band_h), radius=10, fill=FIELD + "55"),
            position=Position(x=X, y=band),
        ),
        layer(
            Rectangle(size=(map_w, band_h), radius=10, fill=BLUE + "77"),
            position=Position(x=map_x, y=band),
        ),
        mono("struct", X + 24, band + 47, 30, weight=600),
        mono("map", end - 24, band + 47, 30, weight=600, align="end"),
        *(
            mono(str(k), px(k), band + 112, 26, fill=DIM, align="middle")
            for k in range(0, AXIS_KEYS + 1, 5)
        ),
        mono("default", px(20), band + 142, 24, fill=DIM, align="middle"),
        box(px(20) - 2, band, 4, band_h, DIM, radius=2),
        layer(
            Rectangle(size=(6, band_h + 36), radius=3, fill=FG),
            mono("map_threshold", 3, -14, 28, align="middle"),
            position=Position(x=map_x, y=band - 18),
        ),
        layer(
            Circle(radius=16, fill=FG),
            position=Position(x=px(n) - 16, y=band + band_h / 2 - 16),
        ),
        mono(f"{n} keys", px(n), band - 26, 28, align="middle"),
    ]
    items.append(enter(line, s.cue(3)))

    # The call gains `, map_threshold=5` in place, its `)` moving over, as the slider
    # moves; an underline draws in under the new argument
    size, cw, top = 28, 28 * 0.6, 770
    head, added, tail = (
        'df.genson.infer_polars_schema("json"',
        f", map_threshold={THRESHOLD}",
        ")",
    )
    card_h, base = size * 1.45 + 32, top + 16 + size
    at = X + 28 + len(head) * cw  # where `added` goes
    grow = dict(duration=1.4, start_at=glide, easing="ease_in_out")
    width = (len(head) + len(tail)) * cw + 56
    added_w = len(added) * cw
    underline = Rectangle(
        size=(
            Tween(
                from_value=0.5,
                to_value=added_w,
                duration=0.6,
                start_at=glide + 0.9,
                easing="ease_out",
            ),
            4,
        ),
        radius=2,
        fill=FIELD,
    )
    call = [
        layer(
            Rectangle(
                size=(
                    Tween(from_value=width, to_value=width + added_w, **grow),
                    card_h,
                ),
                radius=14,
                fill=PANEL,
                stroke=Stroke(color=EDGE, width=2),
            ),
            position=Position(x=X, y=top),
        ),
        mono(head, X + 28, base, size, fill=CODE),
        layer(
            mono(tail, 0, base, size, fill=CODE),
            position=Position(
                x=Tween(from_value=at, to_value=at + added_w, **grow), y=0
            ),
        ),
        enter(mono(added, at, base, size, weight=600, fill=FIELD), glide + 0.4, dy=0),
        layer(underline, position=Position(x=at, y=base + 10)),
    ]
    result_x = 1180
    items += [
        enter(call, s.cue(3)),
        enter(mono("labels:", result_x, base, 32, weight=600), s.cue(4)),
        enter(
            leave(
                mono(
                    f"Struct({d['default_fields']} fields)",
                    result_x + 170,
                    base,
                    32,
                    weight=600,
                    fill=FIELD,
                ),
                glide,
            ),
            s.cue(4),
        ),
        enter(
            mono(d["map_dtype"], result_x + 170, base, 32, weight=600, fill=BLUE),
            glide + 1.2,
            dy=0,
        ),
    ]
    return scene(s, items)


def functions_scene(s, d):
    """The cities' labels on the left; on the right, one function's result at a time."""
    tint = dict(zip(d["langs"], TINTS))
    top, step, h = 300, 110, 60
    rows, places = data_rows(d, top, step, size=28, h=h)
    items = [heading("The map namespace"), enter(rows, s.cue(0))]
    rx, rw = 1360, 400
    results = (
        ("en", d["get_en"], s.cue(1)),
        ("es", d["get_es"], s.cue(2)),
        (None, d["len"], s.cue(3)),
    )
    for k, (key, values, at) in enumerate(results):
        if (
            key
        ):  # labels.map.get([key]), the key as on the left, and those keys outlined
            prefix = "labels.map.get("
            hx = rx + mono_width(len(prefix), 30)
            chip_items, w = key_chip(key, hx + 4, top - 72, tint[key], size=28, h=46)
            column = [
                mono(prefix, rx, top - 38, 30, fill=CODE),
                *chip_items,
                mono(")", hx + w + 8, top - 38, 30, fill=CODE),
            ]
            column += [
                outline(px_, py, pw, h)
                for (_, k_), (px_, py, pw) in places.items()
                if k_ == key
            ]
            for i, value in enumerate(values):
                column += cell(value, rx, top + i * step, rw, h)
        else:  # labels.map.len(): each label flashes as it's counted; a row's total turns blue
            column = [mono("labels.map.len()", rx, top - 38, 30, fill=CODE)]
            for i, value in enumerate(values):
                y = top + i * step
                chips = [place for (row, _), place in places.items() if row == i]
                ticks = [at + 0.6 + j * 0.4 for j in range(len(chips))]
                column.append(box(rx, y, rw, h, PANEL, radius=10, edge=EDGE))
                column += [
                    flash(outline(*place, h), t) for place, t in zip(chips, ticks)
                ]
                shown_from = [at, *ticks, s.duration]
                for count in range(value + 1):
                    done = count == value
                    number = mono(
                        str(count),
                        rx + 20,
                        y + h / 2 + 30 * 0.35,
                        30,
                        weight=600 if done else 400,
                        fill=BLUE if done else FG,
                    )
                    column.append(
                        layer(number).at(
                            shown_from[count],
                            duration=shown_from[count + 1] - shown_from[count],
                        )
                    )
        shown = enter(column, at, dy=0)
        if k + 1 < len(results):
            shown = leave([shown], results[k + 1][2] - 0.35)
        items.append(shown)
    return scene(s, items)


# canvas-confetti's "Realistic Look": five bursts of one shot, as (share of the
# particles, options); and its defaults
CONFETTI_BURSTS = (
    (0.25, dict(spread=26, start_velocity=55)),
    (0.2, dict(spread=60)),
    (0.35, dict(spread=100, decay=0.91, scalar=0.8)),
    (0.1, dict(spread=120, start_velocity=25, decay=0.92, scalar=1.2)),
    (0.1, dict(spread=120, start_velocity=45)),
)
CONFETTI_DEFAULTS = dict(
    angle=90, spread=45, start_velocity=45, decay=0.9, gravity=1, ticks=200, scalar=1
)
CONFETTI_COLORS = (
    "#26ccff",
    "#a25afd",
    "#ff5e7e",
    "#88ff5a",
    "#fcff42",
    "#ffa62d",
    "#ff36ff",
)


def confetti(x0, y0, at, count=200, scale=1.4, seed=7):
    """A canvas-confetti "Realistic Look" shot from (x0, y0) at time `at`.

    A port of canvas-confetti's particle update (catdad/canvas-confetti, `updateFetti`):
    run at its 60 ticks a second and sampled at the video's frame rate, each piece is the
    same wobbling, tilting four-point shape, fading out over its ticks. `scale` sizes
    the whole effect up from browser pixels to the 1080p frame.
    """
    rng = random.Random(seed)
    ticks_per_frame = 60 // FPS
    items = []
    for share, opts in CONFETTI_BURSTS:
        o = {**CONFETTI_DEFAULTS, **opts}
        for _ in range(int(count * share)):
            angle = -math.radians(o["angle"]) + math.radians(o["spread"]) * (
                0.5 - rng.random()
            )
            velocity = o["start_velocity"] * 0.5 + rng.random() * o["start_velocity"]
            wobble, wobble_speed = (
                rng.random() * 10,
                min(0.11, rng.random() * 0.1 + 0.05),
            )
            tilt = (rng.random() * 0.5 + 0.25) * math.pi
            x, y = 0.0, 0.0
            points = [[] for _ in range(8)]  # x and y of the shape's four corners
            for tick in range(o["ticks"]):
                x += math.cos(angle) * velocity
                y += math.sin(angle) * velocity + o["gravity"] * 3
                velocity *= o["decay"]
                wobble += wobble_speed
                wobble_x = x + 10 * o["scalar"] * math.cos(wobble)
                wobble_y = y + 10 * o["scalar"] * math.sin(wobble)
                tilt += 0.1
                r = rng.random() + 2
                x1, y1 = x + r * math.cos(tilt), y + r * math.sin(tilt)
                x2, y2 = wobble_x + r * math.cos(tilt), wobble_y + r * math.sin(tilt)
                if tick % ticks_per_frame == 0:
                    for k, v in enumerate((x, y, wobble_x, y1, x2, y2, x1, wobble_y)):
                        points[k].append(
                            x0 + v * scale if k % 2 == 0 else y0 + v * scale
                        )
            s = [Samples(values=tuple(p), fps=FPS) for p in points]
            shape = VectorPath(
                size=(W, H),
                segments=(
                    MoveTo(x=s[0], y=s[1]),
                    LineTo(x=s[2], y=s[3]),
                    LineTo(x=s[4], y=s[5]),
                    LineTo(x=s[6], y=s[7]),
                    Close(),
                ),
                fill=rng.choice(CONFETTI_COLORS),
            )
            life = o["ticks"] / 60
            fade = Tween(from_value=1, to_value=0, duration=life)
            items.append(layer(shape, opacity=fade).at(at, duration=life))
    return items


def end_scene(s, d):
    """polars-genson, centred; 1.0 slides in beside it, with confetti; then the links."""
    bold = MEDIA / "IBMPlexSans-Bold.ttf"
    size, gap, base = 110, 32, 300
    name_w, version_w = width("polars-genson", bold, size), width("1.0", bold, size)
    alone = (W - name_w) / 2  # polars-genson centred on its own
    paired = (W - name_w - gap - version_w) / 2  # and centred with 1.0 beside it
    version_in = s.cue(0) + 0.8
    shift = dict(duration=0.8, start_at=version_in, easing="ease_out")
    name = layer(
        text("polars-genson", 0, base, size=size, weight=700),
        position=Position(x=Tween(from_value=alone, to_value=paired, **shift), y=0),
    )
    version = layer(
        text("1.0", 0, base, size=size, weight=700, fill=BLUE),
        position=Position(
            x=Tween(from_value=W, to_value=paired + name_w + gap, **shift), y=0
        ),
        opacity=Tween(from_value=0, to_value=1, duration=0.3, start_at=version_in),
    )
    # "for [Polars] 2.0", centred
    regular = MEDIA / "IBMPlexSans-Regular.ttf"
    logo_w, logo_h = 263, 62
    for_w, two_w = (
        width("for ", regular, 40),
        width("2.0", MEDIA / "IBMPlexSans-SemiBold.ttf", 48),
    )
    left = (W - for_w - logo_w - 16 - two_w) / 2
    install = "pip install polars-genson"
    box_w = mono_width(len(install), 52) + 80
    return scene(
        s,
        (
            enter(name, 0),
            version,
            *confetti(W / 2, 0.7 * H, version_in + 0.3),
            enter(
                (
                    text("for", left, 430, size=40, fill=DIM),
                    logo(
                        "polars_logo_white_text.png", left + for_w, 378, logo_w, logo_h
                    ),
                    text(
                        "2.0",
                        left + for_w + logo_w + 16,
                        430,
                        size=48,
                        weight=600,
                        fill=BLUE,
                    ),
                ),
                s.cue(0) + 1.8,
            ),
            enter(
                (
                    panel((W - box_w) / 2, 510, box_w, 104),
                    mono(
                        install, W / 2, 580, 52, weight=600, fill=BLUE, align="middle"
                    ),
                ),
                s.cue(1),
            ),
            enter(
                (
                    text("Docs", 620, 740, size=40, weight=600, fill=BLUE),
                    text("polars-genson.vercel.app", 780, 740, size=40),
                    text("Code", 620, 820, size=40, weight=600, fill=BLUE),
                    text("github.com/lmmx/polars-genson", 780, 820, size=40),
                ),
                s.cue(2),
            ),
        ),
    )


SCENES = {
    "Title": title_scene,
    "Records": records_scene,
    "Maps": maps_scene,
    "As a struct": struct_scene,
    "As a map": map_scene,
    "Map keys": keys_scene,
    "Naming maps": naming_scene,
    "Inferring maps": inferring_scene,
    "Map functions": functions_scene,
    "End card": end_scene,
}


def build(scenes):
    """The video, its scenes timed by `scenes` (from `voiceover.timeline`)."""
    d = compute()
    layers = tuple(SCENES[s.name](s, d) for s in scenes)
    end = scenes[-1].start + scenes[-1].duration
    glow = RadialGradient(stops=(Stop(offset=0, color=GLOW), Stop(offset=1, color=BG)))
    background = Rectangle(size=(W, H), fill=glow)
    # Faint grain over the glow, so its shades dither rather than band once encoded
    grain = Rectangle(
        size=(W, H), fill=Pattern(source=str(MEDIA / "grain.png"), size=(256, 256))
    )
    return Video(
        composition=Composition(duration=end, children=(background, grain, *layers)),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """Each scene a second after its last `say` row starts."""
    return [
        s.start + min(s.duration - 0.4, s.cue(len(s.shards) - 1) + 1.0) for s in scenes
    ]
