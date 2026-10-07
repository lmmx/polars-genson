"""The explainer: struct and Map columns, and how polars-genson infers maps from JSON.

Each scene follows a scene of voiceover.txt, its visuals cued to the shards of the
voiceover. Every value on screen is computed with Polars and polars-genson when the
script runs. Fonts (IBM Plex, OFL) and the Polars logos are in video/media. Render with
render.py.
"""

import io
import tempfile
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401  (registers the .genson namespace)
from fframes.compose import (
    Circle,
    Composition,
    Image,
    Position,
    Rectangle,
    Text,
    TextRun,
    Tween,
    Video,
)
from polars_genson import normalise_from_parquet

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"

W, H, FPS = 1920, 1080, 30
X = 160  # left margin

BG, PANEL, FG, DIM = "#0B0F19", "#161C2A", "#E8ECF4", "#8A93A6"
BLUE, CODE, NULL = "#0075FF", "#6CB6FF", "#E5534B"
FIELD = "#3DDC84"  # struct fields
# One pastel per language, in the order of the struct's fields
TINTS = ("#4C9AFF", "#F2A541", "#E86A92", "#B48CF2", "#4FD1C5", "#F6E05E", "#FF8A65", "#A3B8FF")

PEOPLE = [
    '{"name": "Ada Lovelace", "born": "1815-12-10"}',
    '{"name": "Alan Turing", "born": "1912-06-23"}',
]
# Labels from Wikidata, a few languages each
CITIES = [
    '{"id": "Q64", "labels": {"en": "Berlin", "de": "Berlin", "fr": "Berlin", "pl": "Berlin"}}',
    '{"id": "Q90", "labels": {"en": "Paris", "fr": "Paris", "es": "París", "it": "Parigi", "pt": "Paris"}}',
    '{"id": "Q220", "labels": {"en": "Rome", "it": "Roma"}}',
    '{"id": "Q1492", "labels": {"en": "Barcelona", "ca": "Barcelona", "es": "Barcelona"}}',
]
THRESHOLD = 5  # the map_threshold the video lowers to
AXIS_KEYS = 25  # the number line runs from 0 to this many distinct keys
LIST_LOOKUP = """pl.col("labels")
  .list.eval(
    pl.element().filter(
      pl.element().struct.field("key") == "es"
    )
  )
  .list.first()
  .struct.field("value")"""
MAP_LOOKUP = 'pl.col("labels").map.get("es")'


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
    forced = df.genson.infer_polars_schema("json", force_field_types={"labels": "map"})["labels"]
    assert isinstance(default, pl.Struct), default
    assert lowered == forced == pl.Map(pl.String, pl.String), (lowered, forced)

    cities = df.genson.normalise_json("json", map_threshold=THRESHOLD)
    as_list = df.genson.normalise_json("json", map_threshold=THRESHOLD, map_encoding="kv")
    labels = pl.col("labels")
    # The lookups run as shown on screen
    by_map = cities.select(eval(f"({MAP_LOOKUP})")).to_series().to_list()
    by_list = as_list.select(eval(f"({LIST_LOOKUP})")).to_series().to_list()
    assert by_map == by_list, (by_map, by_list)
    functions = cities.select(
        labels.map.len().alias("len"), labels.map.contains_key("de").alias("de")
    )

    try:
        pl.Series([None], dtype=pl.Map(pl.Object, pl.String))
        object_error = None
    except Exception as e:  # Polars refuses Object keys
        object_error = str(e).splitlines()[0]

    with tempfile.TemporaryDirectory() as tmp:
        src, out = Path(tmp, "cities.parquet"), Path(tmp, "typed.parquet")
        df.write_parquet(src)
        normalise_from_parquet(src, "json", out, map_threshold=THRESHOLD, typed=True)
        parquet_dtype = pl.read_parquet_schema(out)["json"]

    return {
        "people": people,
        "ids": cities["id"].to_list(),
        "langs": grid.columns,
        "grid": grid.rows(),
        "maps": cities["labels"].to_list(),
        "default_fields": len(default.fields),
        "map_dtype": str(lowered),
        "get": by_map,
        "len": functions["len"].to_list(),
        "de": functions["de"].to_list(),
        "object_error": object_error,
        "parquet_dtype": str(parquet_dtype),
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


def box(x, y, w, h, fill, radius=14):
    """A rounded rectangle with its top-left corner at (x, y)."""
    return layer(Rectangle(size=(w, h), radius=radius, fill=fill), position=Position(x=x, y=y))


def logo(name, x, y, w, h):
    """A Polars logo from video/media."""
    image = Image(source=str(MEDIA / name), size=(w, h), fit="contain")
    return layer(image, position=Position(x=x, y=y))


def enter(items, at, *, dy=30):
    """`items` rising `dy` px and fading in at local time `at`."""
    if not isinstance(items, (list, tuple)):
        items = (items,)
    fade = Tween(from_value=0, to_value=1, duration=0.4, start_at=at, easing="ease_out")
    rise = Tween(from_value=dy, to_value=0, duration=0.5, start_at=at, easing="ease_out")
    return layer(*items, opacity=fade, position=Position(x=0, y=rise))


def leave(items, at):
    """`items` fading out at local time `at`."""
    if not isinstance(items, (list, tuple)):
        items = (items,)
    fade = Tween(from_value=1, to_value=0, duration=0.3, start_at=at, easing="ease_in")
    return layer(*items, opacity=fade)


def scene(s, children, *, fade_in=True):
    """Scene `s` of the timeline, fading in and out."""
    fade_out = Tween(from_value=1, to_value=0, duration=0.3, start_at=s.duration - 0.3, easing="ease_in")
    inner = layer(*children, opacity=fade_out)
    if fade_in:
        inner = layer(inner, opacity=Tween(from_value=0, to_value=1, duration=0.4, easing="ease_out"))
    return inner.at(s.start, duration=s.duration)


def heading(words):
    return enter(text(words, X, 170, size=56, weight=600), 0)


def caption(content, at, y=940):
    return enter(text(content, X, y, size=38, fill=DIM), at)


def code_card(lines, x, y, *, size=34, fill=CODE):
    """Code in a panel, top-left at (x, y); `lines` is a string, split on newlines."""
    lines = lines.splitlines()
    leading = size * 1.45
    w = mono_width(max(len(line) for line in lines), size) + 56
    h = leading * len(lines) + 32
    items = [box(x, y, w, h, PANEL)]
    for i, line in enumerate(lines):
        items.append(mono(line, x + 28, y + 16 + size + i * leading, size, fill=fill))
    return items


def json_line(line, x, y, key_color, size=34):
    """A JSON object on one line, its keys coloured by `key_color(key)`."""
    parts, rest = [], line
    while '"' in rest:
        before, _, after = rest.partition('"')
        token, _, rest = after.partition('"')
        parts.append((before, FG))
        is_key = rest.lstrip().startswith(":")
        parts.append((f'"{token}"', key_color(token) if is_key else "#B9C2D3"))
    parts.append((rest, FG))
    return mono(tuple(parts), x, y, size)


def chip(key, value, x, y, tint, size=32):
    """A map entry: the key on its tint, the value beside it. Returns (items, width)."""
    pad, h = 16, 60
    kw = mono_width(len(key), size) + 2 * pad
    vw = mono_width(len(value), size) + 2 * pad
    base = y + h / 2 + size * 0.35
    items = [
        box(x, y, kw + vw, h, PANEL),
        box(x, y, kw, h, tint),
        mono(key, x + pad, base, size, weight=600, fill=BG),
        mono(value, x + kw + pad, base, size),
    ]
    return items, kw + vw


def cell(value, x, y, w, h=64, size=30, fill=PANEL, color=FG):
    """A table cell holding `value`, with null shown in red."""
    shown = "null" if value is None else str(value)
    color = NULL if value is None else color
    return [box(x, y, w, h, fill, radius=10), mono(shown, x + 20, y + h / 2 + size * 0.35, size, fill=color)]


# ---------------------------------------------------------------- scenes


def title_scene(s, d):
    return scene(s, (
        text("JSON → pl.Map", W / 2, 470, size=120, weight=700, align="middle"),
        enter(text("polars-genson 1.0", W / 2, 570, size=52, fill=DIM, align="middle"), 0.3),
        enter(text((("Map: ", BLUE), ("keys that are data", FG)), W / 2, 740, size=44, align="middle"), s.cue(1)),
        enter(text((("Struct: ", FIELD), ("keys that are part of the type", FG)), W / 2, 820, size=44, align="middle"), s.cue(2)),
    ), fade_in=False)


def records_scene(s, d):
    people = d["people"]
    items = [heading("Fields: keys that are part of the type")]
    width = mono_width(max(map(len, PEOPLE)), 32) + 56
    for i, line in enumerate(PEOPLE):
        y = 290 + i * 96
        items.append(enter((box(X, y - 52, width, 76, PANEL), json_line(line, X + 28, y, lambda k: FIELD, 32)), s.cue(0) + i * 0.3))
    # The same people as a typed table: a header of fields, then their dtypes
    widths, top = (440, 320), 470
    xs = (X, X + widths[0] + 8)
    names = people.columns
    dtypes = {pl.String: "str", pl.Date: "date"}
    header = []
    for name, x, w in zip(names, xs, widths):
        header += [box(x, top, w, 64, FIELD, radius=10), mono(name, x + 20, top + 43, 30, weight=600, fill=BG)]
    items.append(enter(header, s.cue(1)))
    items.append(enter([mono(dtypes[people.schema[n]], x + 20, top + 104, 28, fill=FIELD) for n, x in zip(names, xs)], s.cue(3)))
    body = []
    for i, row in enumerate(people.rows()):
        y = top + 124 + i * 72
        for value, x, w in zip(row, xs, widths):
            body += cell(value, x, y, w)
    items.append(enter(body, s.cue(2)))
    struct = str(pl.Struct(people.schema))
    items.append(enter(mono(((struct, FIELD),), X, 880, 40, weight=600), s.cue(6)))
    return scene(s, items)


def maps_scene(s, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [
        heading("Maps: keys that are data"),
        enter(text("Source: Wikidata", W - X, 170, size=30, fill=DIM, align="end"), s.cue(1)),
    ]
    for i, (id_, row) in enumerate(zip(d["ids"], CITIES)):
        y = 320 + i * 96
        labels = row[row.index('"labels": ') + len('"labels": '):-1]
        items.append(enter((
            mono(id_, X, y, 32, fill=DIM),
            json_line(labels, X + 180, y, lambda k: tint.get(k, FG), 30),
        ), s.cue(1) + i * 0.35))
    n = len(d["langs"])
    items.append(caption(f"{n} languages here, out of thousands in Wikidata", s.cue(3)))
    return scene(s, items)


def struct_scene(s, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [heading("As a struct: a field per language")]
    gx, gy, cw, ch, gap = X + 180, 260, 160, 60, 8
    for j, lang in enumerate(d["langs"]):
        x = gx + j * (cw + gap)
        column = [box(x, gy, cw, ch, tint[lang], radius=10), mono(lang, x + cw / 2, gy + 41, 30, weight=600, fill=BG, align="middle")]
        for i, row in enumerate(d["grid"]):
            y = gy + (i + 1) * (ch + gap)
            fill = "#1A1F2B" if row[j] is None else tint[lang] + "33"
            column += cell(row[j], x, y, cw, ch, 24, fill)
        items.append(enter(column, s.cue(1) + j * 0.15))
    for i, id_ in enumerate(d["ids"]):
        items.append(enter(mono(id_, X, gy + (i + 1) * (ch + gap) + 40, 30, fill=DIM), s.cue(0)))
    nulls = sum(v is None for row in d["grid"] for v in row)
    cells = len(d["grid"]) * len(d["langs"])
    items.append(caption(((f"{nulls} of {cells}", NULL), (" values are null", DIM)), s.cue(2), y=720))
    fields = ", ".join(f"'{lang}': String" for lang in d["langs"][:3])
    items.append(enter(mono(((f"Struct({{{fields}, …}})", FIELD),), X, 840, 34), s.cue(3)))
    items.append(caption("A new language means a new field", s.cue(3) + 0.6))
    return scene(s, items)


def map_scene(s, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [heading("As a map: no extra nulls")]
    for i, (id_, labels) in enumerate(zip(d["ids"], d["maps"])):
        y = 260 + i * 92
        row = [mono(id_, X, y + 41, 30, fill=DIM)]
        x = X + 180
        for key, value in labels.items():
            chip_items, width = chip(key, value, x, y, tint[key])
            row += chip_items
            x += width + 14
        items.append(enter(row, s.cue(1) + i * 0.3, dy=0))
    items.append(enter((
        logo("polars_logo_blue.png", X, 690, 200, 100),
        mono((("labels: ", FG), (d["map_dtype"], BLUE)), X + 240, 760, 48, weight=600),
    ), s.cue(2)))
    items.append(caption("The same type, whatever the languages", s.cue(3)))
    return scene(s, items)


def keys_scene(s, d):
    items = [
        heading("Map keys"),
        enter(mono((("pl.Map(", FG), ("key", BLUE), (", value)", FG)), X, 330, 56, weight=600), s.cue(0)),
    ]
    x = X
    for i, dtype in enumerate(("String", "Int64", "Date", "Boolean", "Categorical")):
        w = mono_width(len(dtype), 32) + 40
        items.append(enter((box(x, 390, w, 64, PANEL, radius=10), mono(dtype, x + 20, 433, 32)), s.cue(0) + 0.6 + i * 0.2))
        x += w + 14
    w = mono_width(len("Object"), 32) + 40
    items.append(enter((
        box(x, 390, w, 64, "#3A1F24", radius=10),
        mono("Object", x + 20, 433, 32, fill=NULL),
        box(x + 10, 420, w - 20, 4, NULL, radius=2),
    ), s.cue(2)))
    if d["object_error"]:
        items.append(enter(mono(d["object_error"], X, 520, 24, fill=DIM), s.cue(2) + 0.4))
    items.append(enter(mono((("polars-genson: ", FG), ("Map(String, V)", BLUE)), X, 700, 44, weight=600), s.cue(3)))
    items.append(caption("JSON keys are strings, so these are too, for now", s.cue(4), y=790))
    return scene(s, items)


def inferring_scene(s, d):
    glide = s.cue(9)  # "until you lower it"
    before = 'df.genson.infer_polars_schema("json")'
    after = f'df.genson.infer_polars_schema("json", map_threshold={THRESHOLD})'
    items = [
        heading("How polars-genson infers maps"),
        enter(leave(code_card(before, X, 250), glide), s.cue(0)),
        enter(leave(mono((("labels: ", FG), (f"Struct({d['default_fields']} fields)", FIELD)), X, 400, 40), glide), s.cue(1)),
        enter(code_card(after, X, 250), glide + 0.3, dy=0),
        enter(mono((("labels: ", FG), (d["map_dtype"], BLUE)), X, 400, 40), glide + 1.2, dy=0),
        enter(text("1.  more distinct keys than map_threshold", X, 490, size=36), s.cue(3)),
        enter(text("2.  one type for all the values", X, 550, size=36), s.cue(4)),
    ]

    # A number line of distinct keys: struct up to the threshold, map beyond it
    def px(keys):
        return X + keys * (W - 2 * X) / AXIS_KEYS

    band, band_h, end = 650, 70, px(AXIS_KEYS)
    move = dict(duration=1.2, start_at=glide, easing="ease_in_out")
    struct_w = Tween(from_value=px(20) - X, to_value=px(THRESHOLD) - X, **move)
    map_x = Tween(from_value=px(20), to_value=px(THRESHOLD), **move)
    map_w = Tween(from_value=end - px(20), to_value=end - px(THRESHOLD), **move)
    marker_x = Tween(from_value=px(20), to_value=px(THRESHOLD), **move)
    n = len(d["langs"])
    line = [
        layer(Rectangle(size=(struct_w, band_h), radius=10, fill=FIELD + "55"), position=Position(x=X, y=band)),
        layer(Rectangle(size=(map_w, band_h), radius=10, fill=BLUE + "77"), position=Position(x=map_x, y=band)),
        mono("struct", X + 24, band + 47, 30, weight=600),
        mono("map", end - 24, band + 47, 30, weight=600, align="end"),
        *(mono(str(k), px(k), band + 120, 26, fill=DIM, align="middle") for k in range(0, AXIS_KEYS + 1, 5)),
        layer(
            Rectangle(size=(6, band_h + 40), radius=3, fill=FG),
            mono("map_threshold", 3, -16, 28, align="middle"),
            position=Position(x=marker_x, y=band - 20),
        ),
        layer(Circle(radius=16, fill=FG), position=Position(x=px(n) - 16, y=band + band_h / 2 - 16)),
        mono(f"{n} keys", px(n), band - 40, 28, align="middle"),
    ]
    items.append(enter(line, s.cue(5)))
    return scene(s, items)


def options_scene(s, d):
    call = 'df.genson.normalise_json("json", force_field_types={"labels": "map"})'
    items = [
        heading("Other ways to get a map"),
        enter(code_card(call, X, 250, size=32), s.cue(1)),
        enter(mono((("labels: ", FG), (d["map_dtype"], BLUE)), X, 400, 40), s.cue(1) + 0.8),
    ]
    options = (
        ("unify_maps", "merge record values with different fields into one map"),
        ("map_max_required_keys", "objects with more always-present keys stay structs"),
    )
    for i, (name, meaning) in enumerate(options):
        y = 540 + i * 90
        items.append(enter((mono(name, X, y, 34, fill=CODE), text(meaning, X + 500, y, size=34, fill=DIM)), s.cue(3) + i * 0.4))
    return scene(s, items)


def why_scene(s, d):
    right = X + 960
    items = [
        heading("Why a Map type"),
        enter(mono("List(Struct({'key': String, 'value': String}))", X, 260, 26, fill=DIM), s.cue(1)),
        enter(code_card(LIST_LOOKUP, X, 290, size=28), s.cue(2)),
        enter(mono(d["map_dtype"], right, 260, 26, fill=DIM), s.cue(3)),
        enter(code_card(MAP_LOOKUP, right, 290, size=28), s.cue(3)),
    ]
    values = []
    for i, value in enumerate(d["get"]):
        values += cell(value, right, 420 + i * 72, 320)
    items.append(enter(values, s.cue(3) + 0.6))
    items.append(caption('map_encoding="kv" still gives the list form', s.cue(3) + 1.4))
    return scene(s, items)


def functions_scene(s, d):
    items = [heading("The map namespace")]
    columns = (
        ('labels.map.get("es")', d["get"], X + 180, 380, s.cue(0)),
        ("labels.map.len()", d["len"], X + 600, 300, s.cue(2)),
        ('labels.map.contains_key("de")', d["de"], X + 940, 560, s.cue(4)),
    )
    for code, values, x, w, at in columns:
        cells = [mono(code, x, 300, 28, fill=CODE)]
        for i, value in enumerate(values):
            cells += cell(value, x, 330 + i * 80, w)
        items.append(enter(cells, at))
    for i, id_ in enumerate(d["ids"]):
        items.append(enter(mono(id_, X, 330 + i * 80 + 43, 30, fill=DIM), s.cue(0)))
    return scene(s, items)


def parquet_scene(s, d):
    call = f'normalise_from_parquet(\n    "cities.parquet", "json", "typed.parquet",\n    map_threshold={THRESHOLD}, typed=True,\n)'
    read = 'pl.read_parquet_schema("typed.parquet")["json"]'
    dtype = d["parquet_dtype"]
    map_at = dtype.index("Map(")
    items = [
        heading("Typed Parquet"),
        enter(code_card(call, X, 250, size=32), s.cue(0)),
        enter(code_card(read, X, 520, size=32), s.cue(1)),
        enter(mono(((dtype[:map_at], FG), (dtype[map_at:map_at + len(d["map_dtype"])], BLUE), (dtype[map_at + len(d["map_dtype"]):], FG)), X, 690, 36), s.cue(1) + 0.8),
        caption("Other Parquet readers, such as pyarrow, see a map too", s.cue(2)),
    ]
    return scene(s, items)


def end_scene(s, d):
    install = "pip install polars-genson"
    width = mono_width(len(install), 52) + 80
    return scene(s, (
        enter(text("polars-genson 1.0", W / 2, 300, size=96, weight=700, align="middle"), 0),
        enter((
            box((W - width) / 2, 380, width, 104, PANEL),
            mono(install, W / 2, 450, 52, weight=600, fill=BLUE, align="middle"),
        ), s.cue(1)),
        enter((
            text("Docs", 620, 620, size=40, weight=600, fill=BLUE),
            text("polars-genson.vercel.app", 780, 620, size=40),
            text("Code", 620, 700, size=40, weight=600, fill=BLUE),
            text("github.com/lmmx/polars-genson", 780, 700, size=40),
        ), s.cue(2)),
        enter(text("An independent plugin for Polars 2.0 and later", W / 2, 860, size=32, fill=DIM, align="middle"), s.cue(2) + 0.6),
    ))


SCENES = {
    "Title": title_scene,
    "Records": records_scene,
    "Maps": maps_scene,
    "As a struct": struct_scene,
    "As a map": map_scene,
    "Map keys": keys_scene,
    "Inferring": inferring_scene,
    "Other ways": options_scene,
    "Why a Map type": why_scene,
    "Map functions": functions_scene,
    "Parquet": parquet_scene,
    "End card": end_scene,
}


def build(scenes):
    """The video, its scenes timed by `scenes` (from `voiceover.timeline`)."""
    d = compute()
    layers = tuple(SCENES[s.name](s, d) for s in scenes)
    end = scenes[-1].start + scenes[-1].duration
    return Video(
        composition=Composition(duration=end, children=(Rectangle(size=(W, H), fill=BG), *layers)),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """Each scene a second after its last shard starts."""
    return [s.start + min(s.duration - 0.4, s.cue(len(s.shards) - 1) + 1.0) for s in scenes]
