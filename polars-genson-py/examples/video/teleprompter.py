"""A karaoke-style teleprompter for recording the explainer's voiceover. Render it with
render.py, at the same pace as the explainer.

Each word lights up when it should be spoken. A 3 second countdown comes first: start
recording before it ends, and skip those 3 seconds when adding the recording to the
video (`Audio(source=..., offset=3)`).
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
from voiceover import LEAD_IN, LEAD_OUT

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"

W, H, FPS = 1920, 1080, 30
X = 160
BG, FG, DIM, FAINT, BLUE = "#0B0F19", "#E8ECF4", "#8A93A6", "#3A4152", "#0075FF"
COUNTDOWN = 3  # seconds before the video's 0:00
SIZE = 72  # spoken line
CHARS = 36  # characters per row of the spoken line

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


def opening(line, chars=60):
    """The start of `line`, cut at a word to at most `chars` characters."""
    if len(line) <= chars:
        return line
    return line[: line.rfind(" ", 0, chars)] + " …"


def layer(*children, **kwargs):
    return Composition(size=(W, H), children=tuple(children), **kwargs)


def weight(word):
    """Relative speaking time of a word: its length, plus a pause after punctuation."""
    pause = 5 if word.endswith((".", ":")) else 3 if word.endswith(",") else 0
    if word == "—":
        pause = 4
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


def line_layer(scene, start, duration, line, next_line):
    """One scene's line, its words lighting up in turn, from the video's time `start`."""
    words = line.split()
    speak_from, speak_to = LEAD_IN, duration - LEAD_OUT
    weights = [weight(w) for w in words]
    per_unit = (speak_to - speak_from) / sum(weights)
    cw = SIZE * 0.6  # IBM Plex Mono advance
    places = rows(words)
    top = 520 - (places[-1][0] * 110) / 2
    items = [
        text(f"{int(start) // 60}:{int(start) % 60:02d}  ·  {scene}", X, 170, size=36, family=SANS, fill=DIM),
    ]
    t = speak_from
    for word, w, (row, col) in zip(words, weights, places):
        x, y = X + col * cw, top + row * 110
        lit = Tween(from_value=0, to_value=1, duration=0.12, start_at=t, easing="ease_out")
        items.append(text(word, x, y, fill=FAINT))
        items.append(layer(text(word, x, y, weight=600), opacity=lit))
        span = w * per_unit
        underline = Rectangle(size=(len(word) * cw, 8), radius=4, fill=BLUE)
        items.append(layer(underline, position=Position(x=x, y=y + 22)).at(t, duration=span))
        t += span
    # The line's time, filling across the bottom
    bar = Rectangle(
        size=(Tween(from_value=0.5, to_value=W - 2 * X, duration=duration), 6),
        radius=3,
        fill=BLUE,
    )
    items.append(layer(Rectangle(size=(W - 2 * X, 6), radius=3, fill=FAINT), position=Position(x=X, y=980)))
    items.append(layer(bar, position=Position(x=X, y=980)))
    if next_line:
        items.append(text("Next: " + opening(next_line), X, 900, size=34, family=SANS, fill=DIM))
    return layer(*items).at(COUNTDOWN + start, duration=duration)


def countdown(first_line):
    """3, 2, 1 before the video's 0:00, with the first line shown to read ahead."""
    numbers = [
        layer(text(str(n), W / 2, 600, size=240, family=SANS, weight=700, fill=BLUE, align="middle")).at(i, duration=1)
        for i, n in enumerate(range(COUNTDOWN, 0, -1))
    ]
    first = text("First: " + opening(first_line), X, 900, size=34, family=SANS, fill=DIM)
    return layer(*numbers, first).at(0, duration=COUNTDOWN)


def build(scenes):
    """The teleprompter for `scenes` (from `voiceover.timeline`)."""
    lines = [
        line_layer(scene, start, duration, line, scenes[i + 1][3] if i + 1 < len(scenes) else None)
        for i, (scene, start, duration, line) in enumerate(scenes)
    ]
    clock = layer(
        text(TextTemplate(template="{seconds:.1f}s"), W - X, 170, size=36, family=MONO, fill=DIM, align="end")
    ).at(COUNTDOWN)
    end = scenes[-1][1] + scenes[-1][2]
    return Video(
        composition=Composition(
            duration=COUNTDOWN + end,
            children=(Rectangle(size=(W, H), fill=BG), countdown(scenes[0][3]), *lines, clock),
        ),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """The countdown, then each scene halfway through its line."""
    return [1.5, *(COUNTDOWN + start + duration / 2 for _, start, duration, _ in scenes)]
