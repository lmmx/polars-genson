"""A karaoke teleprompter, after Apple Music's lyrics view, for recording the voiceover.

The `say` rows of script.md run down the screen. The line to speak is bright; each of
its words fills with white from left to right over the time it should take to say it.
The list holds still while a line is spoken and glides up to the next line as it
starts. A gap of GAP_DOTS seconds or more (the countdown, and the pauses between
scenes) gets its own row of three dots, which fill in turn: the next line starts as
the last one fills. The lamp by the reading line is green while you should speak.

A countdown comes first: start recording before it ends, and skip it when adding the
recording to the explainer (`Audio(source=..., offset=COUNTDOWN)`). Render with
render.py.
"""

from pathlib import Path

from fontmetrics import width
from fframes.compose import (
    Circle,
    Composition,
    LinearGradient,
    Position,
    Rectangle,
    Samples,
    Stop,
    Text,
    TextTemplate,
    Tween,
    Video,
)

HERE = Path(__file__).parent
MEDIA = HERE / "media"
FONTS = tuple(str(p) for p in sorted(MEDIA.glob("*.ttf")))
SANS, MONO = "IBM Plex Sans", "IBM Plex Mono"
LINE_FONT = MEDIA / "IBMPlexSans-SemiBold.ttf"

W, H, FPS = 1920, 1080, 30
X = 200
BG, FG, DIM, UNSUNG, GREEN, BLUE = (
    "#0B0F19",
    "#FFFFFF",
    "#8A93A6",
    "#4D566B",
    "#3DDC84",
    "#2F8CFF",
)
COUNTDOWN = 3  # seconds before the explainer's 0:00
SIZE, LINE_W, ROW = 64, 1520, 86  # spoken text: font size, wrap width, row height, px
ENTRY_GAP, SCENE_LABEL = 40, 56  # px between entries, and above a scene's first entry
READING_LINE = 400  # top of the entry being spoken
BASELINE = SIZE * 0.4 + 20  # an entry's first baseline, below its top
MIDDLE = BASELINE - SIZE * 0.35  # the middle of its first line of text
GAP_DOTS = 1.2  # seconds of silence that get a row of dots
GLIDE, ARRIVE = (
    0.5,
    0.05,
)  # seconds the list takes to move, ending this long after a start
OTHER_LINES = 0.35  # opacity of lines other than the one being spoken
EDGE = 0.06  # softness of a word's fill edge, as a fraction of the word


def text(content, x, y, *, size=SIZE, family=SANS, weight=600, fill=FG, align="start"):
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


def wrap(words):
    """Each word's (row, x offset), wrapping at LINE_W."""
    space = width(" ", LINE_FONT, SIZE)
    row, x, places = 0, 0.0, []
    for word in words:
        w = width(word, LINE_FONT, SIZE)
        if x and x + w > LINE_W:
            row, x = row + 1, 0.0
        places.append((row, x))
        x += w + space
    return places


def entries(scenes):
    """The column, in order: ("label", scene), ("say", t0, t1, text), ("dots", t0, t1).

    Times are in video seconds. Each gap of GAP_DOTS or more before a `say` row,
    including the countdown before the first, gets a dots entry.
    """
    out, last_end = [], 0.0
    for scene in scenes:
        out.append(("label", scene))
        for shard in scene.shards:
            t0 = COUNTDOWN + scene.start + shard.start
            if t0 - last_end >= GAP_DOTS:
                out.append(("dots", last_end, t0))
            out.append(("say", t0, t0 + shard.duration, shard.text))
            last_end = t0 + shard.duration
    return out


def layout(column):
    """Each entry's top baseline in the column, and the column's height."""
    y, ys = 0.0, []
    for entry in column:
        if entry[0] == "label":
            ys.append(y)
            y += SCENE_LABEL
        elif entry[0] == "dots":
            ys.append(y)
            y += ROW + ENTRY_GAP
        else:
            ys.append(y)
            y += (wrap(entry[3].split())[-1][0] + 1) * ROW + ENTRY_GAP
    return ys, y


def smoothstep(f):
    f = min(1.0, max(0.0, f))
    return f * f * (3 - 2 * f)


def glide(prev_end, start):
    """When the list starts gliding to an entry that starts at `start`, and for how long:
    GLIDE seconds, arriving ARRIVE after the start, but never before `prev_end` (the end
    of the entry being spoken), so it doesn't move under a line."""
    begin = max(prev_end, start + ARRIVE - GLIDE)
    return begin, start + ARRIVE - begin


def scroll(column, ys, end):
    """Per-frame y offset of the column: still while a line is spoken, gliding up to
    each `say` or dots entry as it starts."""
    stops = [
        (entry[1], entry[2], y) for entry, y in zip(column, ys) if entry[0] != "label"
    ]
    moves = [
        (*glide(prev[1], nxt[0]), prev[2], nxt[2])
        for prev, nxt in zip(stops, stops[1:])
    ]
    values, k = [], 0
    for frame in range(int(end * FPS) + 1):
        t = frame / FPS
        while k < len(moves) and t >= moves[k][0] + moves[k][1]:
            k += 1
        if k == len(moves):
            y = stops[-1][2]
        else:
            begin, duration, y_from, y_to = moves[k]
            y = y_from + (y_to - y_from) * smoothstep((t - begin) / duration)
        values.append(READING_LINE - y)
    return Samples(values=tuple(values), fps=FPS)


def highlighted(items, t0, t1):
    """`items` bright from t0 to t1, and at OTHER_LINES opacity otherwise."""
    up = Tween(
        from_value=OTHER_LINES,
        to_value=1,
        duration=GLIDE,
        start_at=max(0.0, t0 + ARRIVE - GLIDE),
        easing="ease_in_out",
    )
    down = Tween(
        from_value=1,
        to_value=OTHER_LINES,
        duration=GLIDE,
        start_at=t1,
        easing="ease_in_out",
    )
    return layer(layer(*items, opacity=down), opacity=up)


def say_items(y, t0, t1, line):
    """A line's words, each filling with white over its share of t0..t1."""
    words = line.split()
    weights = [weight(w) for w in words]
    per_unit = (t1 - t0) / sum(weights)
    items, t = [], t0
    for word, w, (row, x) in zip(words, weights, wrap(words)):
        span = w * per_unit
        # White up to the edge, grey beyond it; the edge crosses the word over its span
        edge = Tween(
            from_value=-0.5 - EDGE, to_value=0.5 + EDGE, duration=span, start_at=t
        )
        fill = LinearGradient(
            start=(0, 0),
            end=(1, 0),
            stops=(
                Stop(offset=0, color=FG),
                Stop(offset=0.5 - EDGE, color=FG),
                Stop(offset=0.5 + EDGE, color=UNSUNG),
                Stop(offset=1, color=UNSUNG),
            ),
            matrix=(1, 0, 0, 1, edge, 0),
        )
        items.append(text(word, X + x, y + row * ROW, fill=fill))
        t += span
    return highlighted(items, t0, t1)


def dots_items(top, t0, t1):
    """Three dots, filling one after another across the gap t0..t1."""
    items, step = [], (t1 - t0) / 3
    for k in range(3):
        cx, cy, r = X + 14 + k * 46, top + MIDDLE, 13
        dim = layer(
            Circle(radius=r, fill=UNSUNG), position=Position(x=cx - r, y=cy - r)
        )
        lit = Tween(from_value=0, to_value=1, duration=step, start_at=t0 + k * step)
        bright = layer(
            Circle(radius=r, fill=FG),
            position=Position(x=cx - r, y=cy - r),
            opacity=lit,
        )
        items += [dim, bright]
    return highlighted(items, t0, t1)


def build(scenes):
    """The teleprompter for `scenes` (from `voiceover.timeline`)."""
    end = COUNTDOWN + scenes[-1].start + scenes[-1].duration
    column = entries(scenes)
    ys, _ = layout(column)
    items = []
    for entry, y in zip(column, ys):
        if entry[0] == "label":
            scene = entry[1]
            clock = f"{int(scene.start) // 60}:{int(scene.start) % 60:02d}"
            items.append(
                text(
                    f"{clock}  ·  {scene.name}",
                    X,
                    y + 24,
                    size=28,
                    weight=400,
                    fill=DIM,
                )
            )
        elif entry[0] == "dots":
            items.append(dots_items(y, entry[1], entry[2]))
        else:
            items.append(say_items(y + BASELINE, *entry[1:]))
    rolling = Composition(
        size=(W, H),
        children=tuple(items),
        position=Position(x=0, y=scroll(column, ys, end)),
    )

    # The lamp: green while a line is being spoken
    lamp_at = Position(x=X - 70, y=READING_LINE + MIDDLE - 14)
    lamp = [layer(Circle(radius=14, fill=UNSUNG), position=lamp_at)]
    lamp += [
        layer(Circle(radius=14, fill=GREEN), position=lamp_at).at(
            entry[1], duration=entry[2] - entry[1]
        )
        for entry in column
        if entry[0] == "say"
    ]
    countdown = [
        layer(
            text(str(n), W - X, 200, size=120, weight=700, fill=BLUE, align="end")
        ).at(i, duration=1)
        for i, n in enumerate(range(COUNTDOWN, 0, -1))
    ]
    clock = layer(
        text(
            TextTemplate(template="{seconds:.1f}s"),
            W - X,
            110,
            size=34,
            family=MONO,
            weight=400,
            fill=DIM,
            align="end",
        )
    ).at(COUNTDOWN)
    return Video(
        composition=Composition(
            duration=end,
            children=(
                Rectangle(size=(W, H), fill=BG),
                rolling,
                *lamp,
                *countdown,
                clock,
            ),
        ),
        resolution=(W, H),
        fps=FPS,
        fonts=FONTS,
        load_system_fonts=False,
    )


def preview_times(scenes):
    """The countdown, then each scene halfway through its first line and in the gap after."""
    times = [1.5]
    for s in scenes:
        first = s.shards[0]
        times.append(COUNTDOWN + s.start + first.start + first.duration / 2)
    return times
