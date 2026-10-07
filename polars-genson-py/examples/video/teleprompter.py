"""A rolling, karaoke-style teleprompter for recording the explainer's voiceover.

The shards of voiceover.txt run down the screen one per line, and roll up steadily so
the shard being spoken passes the reading line (the blue mark) as its words light up.
The scroll is continuous, so a recording that runs a little ahead or behind still has
the lines around it on screen. A countdown comes first: start recording before it
ends, and skip it when adding the recording to the explainer
(`Audio(source=..., offset=COUNTDOWN)`). Render with render.py, at the explainer's pace.
"""

from pathlib import Path

from fframes.compose import (
    Composition,
    Position,
    Rectangle,
    Samples,
    Text,
    TextTemplate,
    Tween,
    Video,
)

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"

W, H, FPS = 1920, 1080, 30
X = 160
BG, FG, DIM, FAINT, BLUE = "#0B0F19", "#E8ECF4", "#8A93A6", "#4A5266", "#0075FF"
COUNTDOWN = 3  # seconds before the explainer's 0:00
SIZE, CHARS = 60, 44  # shard text: font size, characters per row
ROW, SHARD_GAP, SCENE_GAP = 80, 28, 70  # vertical spacing, px
READING_LINE = 460  # baseline of the shard being spoken
SPOKEN = 0.45  # opacity of a shard once spoken


def text(content, x, y, *, size=SIZE, family=MONO, weight=400, fill=FG, align="start"):
    """Text with its baseline at `y`; `content` is a string or a TextTemplate."""
    return Text(
        content=content,
        font_size=size,
        font_family=family,
        font_weight=weight,
        fill=fill,
        anchor="baseline",
        text_anchor=align,
        position=Position(x=x, y=y),
    )


def layer(*children, **kwargs):
    return Composition(size=(W, H), children=tuple(children), **kwargs)


def weight(word):
    """Relative speaking time of a word: its length, plus a pause after punctuation."""
    pause = 4 if word.endswith((".", ":")) else 2 if word.endswith(",") else 0
    return len(word.strip(".,:")) + 2 + pause


def rows(words):
    """Wrap words into rows of at most CHARS characters: (row, column) per word."""
    row, col, places = 0, 0, []
    for word in words:
        if col and col + len(word) > CHARS:
            row, col = row + 1, 0
        places.append((row, col))
        col += len(word) + 1
    return places


def script_layout(scenes):
    """Where every scene label and shard sits in the scrolling column.

    Returns (labels, shards): labels as (y, scene); shards as (y, t0, t1, shard) with
    y its first baseline and t0..t1 when it's spoken, in video seconds.
    """
    y, labels, shards = 0.0, [], []
    for scene in scenes:
        labels.append((y, scene))
        y += SCENE_GAP
        for shard in scene.shards:
            t0 = COUNTDOWN + scene.start + shard.start
            shards.append((y, t0, t0 + shard.duration, shard))
            y += (rows(shard.text.split())[-1][0] + 1) * ROW + SHARD_GAP
        y += SCENE_GAP - SHARD_GAP
    return labels, shards


def scroll(shards, end):
    """Per-frame y offset of the column: shard i reaches the reading line when it
    starts, and the column glides linearly on to shard i + 1."""
    stops = [(t0, y) for y, t0, _, _ in shards]
    stops.append((end, shards[-1][0] + ROW))
    values, k = [], 0
    for frame in range(int(end * FPS) + 1):
        t = frame / FPS
        while k + 1 < len(stops) and stops[k + 1][0] <= t:
            k += 1
        (ta, ya), (tb, yb) = stops[k], stops[min(k + 1, len(stops) - 1)]
        f = 0.0 if t <= ta or tb == ta else min(1.0, (t - ta) / (tb - ta))
        values.append(READING_LINE - (ya + (yb - ya) * f))
    return Samples(values=tuple(values), fps=FPS)


def shard_items(y, t0, t1, shard):
    """A shard's words: faint, lit one by one from t0, then dimmed once spoken."""
    words = shard.text.split()
    weights = [weight(w) for w in words]
    per_unit = (t1 - t0) / sum(weights)
    cw = SIZE * 0.6  # IBM Plex Mono advance
    faint, lit, t = [], [], t0
    for word, w, (row, col) in zip(words, weights, rows(words)):
        x, by = X + col * cw, y + row * ROW
        faint.append(text(word, x, by, fill=FAINT))
        on = Tween(from_value=0, to_value=1, duration=0.15, start_at=t, easing="ease_out")
        lit.append(layer(text(word, x, by, weight=600), opacity=on))
        span = w * per_unit
        underline = Rectangle(size=(len(word) * cw, 6), radius=3, fill=BLUE)
        lit.append(layer(underline, position=Position(x=x, y=by + 18)).at(t, duration=span))
        t += span
    spoken = Tween(from_value=1, to_value=SPOKEN, duration=0.6, start_at=t1 + 0.3, easing="ease_in_out")
    return [*faint, layer(*lit, opacity=spoken)]


def build(scenes):
    """The teleprompter for `scenes` (from `voiceover.timeline`)."""
    end = COUNTDOWN + scenes[-1].start + scenes[-1].duration
    labels, shards = script_layout(scenes)
    column = []
    for y, scene in labels:
        clock = f"{int(scene.start) // 60}:{int(scene.start) % 60:02d}"
        column.append(text(f"{clock}  ·  {scene.name}", X, y + 20, size=30, family=SANS, fill=DIM))
    for entry in shards:
        column += shard_items(*entry)
    rolling = Composition(
        size=(W, H), children=tuple(column), position=Position(x=0, y=scroll(shards, end))
    )
    mark = layer(Rectangle(size=(10, 56), radius=5, fill=BLUE), position=Position(x=X - 50, y=READING_LINE - 46))
    countdown = [
        layer(text(str(n), W - X, 240, size=160, family=SANS, weight=700, fill=BLUE, align="end")).at(i, duration=1)
        for i, n in enumerate(range(COUNTDOWN, 0, -1))
    ]
    clock = layer(
        text(TextTemplate(template="{seconds:.1f}s"), W - X, 120, size=34, family=MONO, fill=DIM, align="end")
    ).at(COUNTDOWN)
    return Video(
        composition=Composition(
            duration=end,
            children=(Rectangle(size=(W, H), fill=BG), rolling, mark, *countdown, clock),
        ),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """The countdown, then each scene a second into its second shard."""
    return [
        1.5,
        *(COUNTDOWN + s.start + s.shards[min(1, len(s.shards) - 1)].start + 1.0 for s in scenes),
    ]
