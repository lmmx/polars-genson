"""The explainer: JSON maps, `pl.Map` and polars-genson. Render it with render.py.

Every value on screen is computed with Polars and polars-genson when the script runs.
Fonts (IBM Plex, OFL) and the Polars logos are in video/media.
"""

import io
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401  (registers the .genson namespace)
from fframes.compose import (
    Composition,
    Image,
    Position,
    Rectangle,
    Text,
    TextRun,
    Tween,
    Video,
)

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"

W, H, FPS = 1920, 1080, 30
X = 160  # left margin

BG, PANEL, FG, DIM = "#0B0F19", "#161C2A", "#E8ECF4", "#8A93A6"
BLUE, CODE, NULL = "#0075FF", "#6CB6FF", "#E5534B"
# One colour per language key, in the order the keys first appear
TINTS = ("#4C9AFF", "#F2A541", "#E86A92", "#57C785", "#B48CF2", "#4FD1C5", "#F6E05E")
FIELD = "#F2A541"  # the record keys share one colour

PEOPLE = ['{"name": "Ada", "born": 1815}', '{"name": "Alan", "born": 1912}']
ROWS = [
    '{"id": "Q64", "labels": {"en": "Berlin", "de": "Berlin", "pl": "Berlin"}}',
    '{"id": "Q90", "labels": {"en": "Paris", "es": "París", "it": "Parigi"}}',
    '{"id": "Q220", "labels": {"en": "Rome", "it": "Roma", "fr": "Rome"}}',
    '{"id": "Q1492", "labels": {"en": "Barcelona", "ca": "Barcelona", "es": "Barcelona"}}',
]
THRESHOLD = 5


# ---------------------------------------------------------------- data


def compute():
    """Everything the video shows, from Polars and polars-genson."""
    df = pl.DataFrame({"json": ROWS})
    plain = pl.read_ndjson(io.StringIO("\n".join(ROWS)))
    grid = plain["labels"].struct.unnest()  # one column per language, nulls elsewhere
    entities = df.genson.normalise_json("json", map_threshold=THRESHOLD)
    dtype = entities.schema["labels"]
    assert dtype == pl.Map(pl.String, pl.String), dtype
    labels = pl.col("labels")
    ops = entities.select(labels.map.get("en"), labels.map.len().alias("len"))
    return {
        "ids": entities["id"].to_list(),
        "langs": grid.columns,
        "grid": grid.rows(),
        "maps": entities["labels"].to_list(),
        "dtype": str(dtype),
        "en": ops["labels"].to_list(),
        "len": ops["len"].to_list(),
    }


# ---------------------------------------------------------------- drawing


def text(content, x, y, *, size=44, family=SANS, weight=400, fill=FG, middle=False):
    """Text with its baseline at `y`; `content` is a string or (text, colour) pairs."""
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
        text_anchor="middle" if middle else "start",
        position=Position(x=x, y=y),
    )


def mono_width(chars, size):
    """Width of `chars` characters of IBM Plex Mono (600 units per em)."""
    return chars * size * 0.6


def box(x, y, w, h, fill, radius=14):
    """A rounded rectangle with its top-left corner at (x, y)."""
    rect = Rectangle(size=(w, h), radius=radius, fill=fill)
    return Composition(size=(W, H), children=(rect,), position=Position(x=x, y=y))


def logo(name, x, y, w, h):
    """A Polars logo from video/media."""
    image = Image(source=str(MEDIA / name), size=(w, h), fit="contain")
    return Composition(size=(W, H), children=(image,), position=Position(x=x, y=y))


def enter(items, at, *, dy=40):
    """`items` rising `dy` px and fading in at local time `at`."""
    if not isinstance(items, (list, tuple)):
        items = (items,)
    fade = Tween(from_value=0, to_value=1, duration=0.4, start_at=at, easing="ease_out")
    rise = Tween(from_value=dy, to_value=0, duration=0.5, start_at=at, easing="ease_out")
    return Composition(
        size=(W, H), children=tuple(items), opacity=fade, position=Position(x=0, y=rise)
    )


def scene(start, duration, children, *, fade_in=True):
    """A scene placed at `start`, fading in and out."""
    fade_out = Tween(
        from_value=1, to_value=0, duration=0.3, start_at=duration - 0.3, easing="ease_in"
    )
    inner = Composition(size=(W, H), children=tuple(children), opacity=fade_out)
    if fade_in:
        fade = Tween(from_value=0, to_value=1, duration=0.4, easing="ease_out")
        inner = Composition(size=(W, H), children=(inner,), opacity=fade)
    return inner.at(start, duration=duration)


def heading(words, at=0.0):
    return enter(text(words, X, 190, size=60, weight=600), at)


def caption(content, at, y=930):
    return enter(text(content, X, y, size=40, fill=DIM), at)


def json_line(line, x, y, key_color, size=36):
    """A JSON object on one line, its keys coloured by `key_color(key)`."""
    parts, rest = [], line
    while '"' in rest:
        before, _, after = rest.partition('"')
        token, _, rest = after.partition('"')
        parts.append((before, FG))
        is_key = rest.lstrip().startswith(":")
        color = key_color(token) if is_key else "#B9C2D3"
        parts.append((f'"{token}"', color))
    parts.append((rest, FG))
    return text(tuple(parts), x, y, size=size, family=MONO)


def chip(key, value, x, y, tint, size=34):
    """A map entry: the key on its tint, the value beside it. Returns (items, width)."""
    pad, h = 18, 64
    kw = mono_width(len(key), size) + 2 * pad
    vw = mono_width(len(value), size) + 2 * pad
    base = y + h / 2 + size * 0.35
    items = (
        box(x, y, kw + vw, h, PANEL),
        box(x, y, kw, h, tint),
        text(key, x + pad, base, size=size, family=MONO, weight=600, fill=BG),
        text(value, x + kw + pad, base, size=size, family=MONO),
    )
    return items, kw + vw


# ---------------------------------------------------------------- scenes


def title_scene(start, duration):
    return scene(start, duration, (
        logo("polars_logo_white_text.png", (W - 379) / 2, 250, 379, 90),
        enter(text("JSON → pl.Map", W / 2, 560, size=120, weight=700, middle=True), 0.2),
        enter(text("with polars-genson", W / 2, 650, size=48, fill=DIM, middle=True), 0.7),
    ), fade_in=False)


def records_scene(start, duration):
    items = [heading("Some JSON objects are records")]
    for i, line in enumerate(PEOPLE):
        y = 360 + i * 110
        items.append(enter((box(X, y - 58, 700, 84, PANEL), json_line(line, X + 28, y, lambda k: FIELD)), 0.6 + i * 0.3))
    # The same rows as a table: one column per key
    tx, cw, ch = 1100, 240, 72
    cells = []
    for j, name in enumerate(("name", "born")):
        cells += [box(tx + j * (cw + 8), 302, cw, ch, FIELD), text(name, tx + j * (cw + 8) + 24, 350, size=34, family=MONO, weight=600, fill=BG)]
    for i, (name, born) in enumerate((("Ada", "1815"), ("Alan", "1912"))):
        y = 302 + (i + 1) * (ch + 8)
        for j, value in enumerate((name, born)):
            cells += [box(tx + j * (cw + 8), y, cw, ch, PANEL), text(value, tx + j * (cw + 8) + 24, y + 48, size=34, family=MONO)]
    items.append(enter(text("→", 900, 420, size=72, fill=DIM), duration * 0.4, dy=0))
    items.append(enter(cells, duration * 0.45))
    items.append(caption((("Every ", DIM), ("key", FIELD), (" has a corresponding column", DIM)), duration * 0.65))
    return scene(start, duration, items)


def maps_scene(start, duration, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [heading("Others are maps: the keys are data")]
    for i, (id_, row) in enumerate(zip(d["ids"], ROWS)):
        y = 340 + i * 110
        labels = row[row.index('"labels": ') + len('"labels": '):-1]
        items.append(enter((
            text(id_, X, y, size=36, family=MONO, fill=DIM),
            json_line(labels, X + 200, y, lambda k: tint.get(k, FG)),
        ), 1.0 + i * 0.6))
    items.append(caption("The languages change from row to row", duration * 0.6))
    return scene(start, duration, items)


def struct_scene(start, duration, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [heading("Read as a struct: a column for every key")]
    gx, gy, cw, ch, gap = X + 200, 290, 176, 72, 8
    for j, lang in enumerate(d["langs"]):
        x = gx + j * (cw + gap)
        header = (box(x, gy, cw, ch, tint[lang]), text(lang, x + cw / 2, gy + 48, size=34, family=MONO, weight=600, fill=BG, middle=True))
        column = list(header)
        for i, row in enumerate(d["grid"]):
            y = gy + (i + 1) * (ch + gap)
            value = row[j]
            if value is None:
                column += [box(x, y, cw, ch, "#1A1F2B"), text("null", x + cw / 2, y + 47, size=28, family=MONO, fill=NULL, middle=True)]
            else:
                column += [box(x, y, cw, ch, tint[lang] + "33"), text(value, x + cw / 2, y + 47, size=28, family=MONO, middle=True)]
        items.append(enter(column, 0.8 + j * 0.25))
    for i, id_ in enumerate(d["ids"]):
        items.append(enter(text(id_, X, gy + (i + 1) * (ch + gap) + 47, size=32, family=MONO, fill=DIM), 0.6))
    nulls = sum(v is None for row in d["grid"] for v in row)
    cells = len(d["grid"]) * len(d["langs"])
    items.append(caption(((f"{nulls} of {cells}", NULL), (" cells are null", DIM)), duration * 0.6))
    return scene(start, duration, items)


def map_scene(start, duration, d):
    tint = dict(zip(d["langs"], TINTS))
    items = [heading("Read as a map: no extra nulls")]
    for i, (id_, labels) in enumerate(zip(d["ids"], d["maps"])):
        y = 280 + i * 100
        row = [text(id_, X, y + 45, size=32, family=MONO, fill=DIM)]
        x = X + 200
        for key, value in labels.items():
            chip_items, width = chip(key, value, x, y, tint[key])
            row += chip_items
            x += width + 16
        items.append(enter(row, 0.8 + i * 0.4, dy=0))
    dtype = d["dtype"]
    items.append(enter((
        logo("polars_logo_blue.png", X, 760, 220, 110),
        text((("labels: ", FG), (dtype, BLUE)), X + 260, 838, size=48, family=MONO, weight=600),
    ), duration * 0.6))
    return scene(start, duration, items)


def genson_scene(start, duration, d):
    tint = dict(zip(d["langs"], TINTS))
    call = f'df.genson.normalise_json("json", map_threshold={THRESHOLD})'
    items = [
        heading("polars-genson works out which are maps"),
        enter((box(X, 250, mono_width(len(call), 36) + 56, 84, PANEL), text(call, X + 28, 306, size=36, family=MONO, fill=CODE)), 0.5),
        enter(text("distinct label keys", X, 450, size=34, fill=DIM), duration * 0.3),
    ]
    cw, gap, y = 136, 16, 490
    keys_at, stagger = duration * 0.35, 0.45
    for j, lang in enumerate(d["langs"]):
        x = X + j * (cw + gap)
        items.append(enter((box(x, y, cw, 80, tint[lang]), text(lang, x + cw / 2, y + 54, size=36, family=MONO, weight=600, fill=BG, middle=True)), keys_at + j * stagger))
    mark = X + THRESHOLD * (cw + gap) - gap / 2
    items.append(enter((box(mark - 3, y - 30, 6, 140, FG, radius=3), text(f"map_threshold = {THRESHOLD}", mark, y + 160, size=32, family=MONO, middle=True)), keys_at - 0.2, dy=0))
    n = len(d["langs"])
    verdict = enter(text(((f"{n} > {THRESHOLD}", FG), ("  →  ", DIM), ("map", BLUE)), X, 800, size=56, family=MONO, weight=600), keys_at + n * stagger + 0.3)
    items.append(verdict)
    items.append(caption("The default threshold is 20 distinct keys", keys_at + n * stagger + 1.5))
    return scene(start, duration, items)


def functions_scene(start, duration, d):
    items = [heading("Then use Polars' map functions")]
    columns = (('labels.map.get("en")', d["en"], X + 200, 420), ("labels.map.len()", d["len"], X + 760, 300))
    for c, (code, values, x, w) in enumerate(columns):
        cells = [text(code, x, 320, size=34, family=MONO, fill=CODE)]
        for i, value in enumerate(values):
            y = 360 + i * 92
            cells += [box(x, y, w, 76, PANEL), text(str(value), x + 24, y + 50, size=36, family=MONO)]
        items.append(enter(cells, duration * (0.35 + c * 0.25)))
    for i, id_ in enumerate(d["ids"]):
        items.append(enter(text(id_, X, 360 + i * 92 + 50, size=32, family=MONO, fill=DIM), 0.6))
    items.append(caption("get, keys, values, len, contains_key", duration * 0.75))
    return scene(start, duration, items)


def outro_scene(start, duration):
    install = "pip install polars-genson"
    width = mono_width(len(install), 56) + 80
    return scene(start, duration, (
        logo("polars_logo_white_text.png", (W - 379) / 2, 230, 379, 90),
        enter((box((W - width) / 2, 420, width, 110, PANEL), text(install, W / 2, 496, size=56, family=MONO, weight=600, fill=BLUE, middle=True)), 0.3),
        enter(text("polars-genson.vercel.app", W / 2, 660, size=44, middle=True), 0.7),
        enter(text("Polars 2  ·  pola.rs", W / 2, 740, size=36, fill=DIM, middle=True), 1.0),
    ))


def build(scenes):
    """The video, its scenes timed by `scenes` (from `voiceover.timeline`)."""
    d = compute()
    t = {scene: (start, duration) for scene, start, duration, _ in scenes}
    layers = (
        title_scene(*t["Title"]),
        records_scene(*t["Records"]),
        maps_scene(*t["Maps"], d),
        struct_scene(*t["As a struct"], d),
        map_scene(*t["As a map"], d),
        genson_scene(*t["genson"], d),
        functions_scene(*t["Map functions"], d),
        outro_scene(*t["End card"]),
    )
    end = sum(duration for _, duration in t.values())
    background = Rectangle(size=(W, H), fill=BG)
    return Video(
        composition=Composition(duration=end, children=(background, *layers)),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """Each scene once everything in it has appeared."""
    return [start + duration * 0.9 for _, start, duration, _ in scenes]
