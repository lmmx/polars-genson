"""A karaoke-style teleprompter for recording the explainer's voiceover.

One shard of voiceover.txt is on screen at a time, its words lighting up when they
should be spoken, with the shard after it faintly below. A countdown comes first:
start recording before it ends, and skip it when adding the recording to the
explainer (`Audio(source=..., offset=COUNTDOWN)`). Render with render.py, at the same
pace as the explainer.
"""

from pathlib import Path

from fframes.compose import (
    Composition,
    Position,
    Rectangle,
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
BG, FG, DIM, FAINT, BLUE = "#0B0F19", "#E8ECF4", "#8A93A6", "#3A4152", "#0075FF"
COUNTDOWN = 3  # seconds before the explainer's 0:00
SIZE, CHARS, ROW = 72, 36, 100  # the shard: font size, characters per row, row height
UPCOMING = 40  # font size of the shard after


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


def shard_layer(shard, upcoming):
    """One shard, its words lighting up across its duration, from its start."""
    words = shard.text.split()
    weights = [weight(w) for w in words]
    per_unit = shard.duration / sum(weights)
    cw = SIZE * 0.6  # IBM Plex Mono advance
    places = rows(words)
    top = 560 - places[-1][0] * ROW / 2
    items, t = [], 0.0
    for word, w, (row, col) in zip(words, weights, places):
        x, y = X + col * cw, top + row * ROW
        lit = Tween(from_value=0, to_value=1, duration=0.12, start_at=t, easing="ease_out")
        items += [text(word, x, y, fill=FAINT), layer(text(word, x, y, weight=600), opacity=lit)]
        underline = Rectangle(size=(len(word) * cw, 8), radius=4, fill=BLUE)
        span = w * per_unit
        items.append(layer(underline, position=Position(x=x, y=y + 22)).at(t, duration=span))
        t += span
    fill = Tween(from_value=0.5, to_value=W - 2 * X, duration=shard.duration)
    items += [
        layer(Rectangle(size=(W - 2 * X, 6), radius=3, fill=FAINT), position=Position(x=X, y=980)),
        layer(Rectangle(size=(fill, 6), radius=3, fill=BLUE), position=Position(x=X, y=980)),
    ]
    if upcoming:
        items.append(text(upcoming, X, 860, size=UPCOMING, fill=FAINT))
    return layer(*items)


def scene_layer(scene, upcoming):
    """A scene's shards in turn, each held until the next one starts."""
    shards = scene.shards
    clock = f"{int(scene.start) // 60}:{int(scene.start) % 60:02d}"
    items = [text(f"{clock}  ·  {scene.name}", X, 170, size=36, family=SANS, fill=DIM)]
    for i, shard in enumerate(shards):
        after = shards[i + 1].text if i + 1 < len(shards) else upcoming
        until = shards[i + 1].start if i + 1 < len(shards) else scene.duration
        items.append(shard_layer(shard, after).at(shard.start, duration=until - shard.start))
    return layer(*items).at(COUNTDOWN + scene.start, duration=scene.duration)


def countdown(first):
    """3, 2, 1 before the explainer's 0:00, with the first shard shown to read ahead."""
    numbers = [
        layer(
            text(str(n), W / 2, 600, size=240, family=SANS, weight=700, fill=BLUE, align="middle")
        ).at(i, duration=1)
        for i, n in enumerate(range(COUNTDOWN, 0, -1))
    ]
    first = text(first, X, 860, size=UPCOMING, fill=FAINT)
    return layer(*numbers, first).at(0, duration=COUNTDOWN)


def build(scenes):
    """The teleprompter for `scenes` (from `voiceover.timeline`)."""
    layers = [
        scene_layer(scene, scenes[i + 1].shards[0].text if i + 1 < len(scenes) else None)
        for i, scene in enumerate(scenes)
    ]
    clock = layer(
        text(TextTemplate(template="{seconds:.1f}s"), W - X, 170, size=36, family=MONO, fill=DIM, align="end")
    ).at(COUNTDOWN)
    end = scenes[-1].start + scenes[-1].duration
    background = Rectangle(size=(W, H), fill=BG)
    return Video(
        composition=Composition(
            duration=COUNTDOWN + end,
            children=(background, countdown(scenes[0].shards[0].text), *layers, clock),
        ),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """The countdown, then each scene half a second into its second shard."""
    return [
        1.5,
        *(COUNTDOWN + s.start + s.shards[min(1, len(s.shards) - 1)].start + 0.5 for s in scenes),
    ]
