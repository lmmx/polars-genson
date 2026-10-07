"""The script (script.md), and the timings of both videos and the captions from it.

script.md is a table read top to bottom: each `say` row is a teleprompter line lasting
its `seconds`, each `pause` row a silence, and each `written` row a scene's paragraph
for the captions. A `say` row with no seconds lasts its words over WORDS_PER_SECOND.
"""

import re
from pathlib import Path
from typing import NamedTuple

SCRIPT = Path(__file__).with_name("script.md")
WORDS_PER_SECOND = 2.7  # for `say` rows with no seconds


class Shard(NamedTuple):
    text: str
    start: float  # seconds from the start of its scene
    duration: float


class Scene(NamedTuple):
    name: str
    written: str
    start: float  # seconds from the start of the video
    duration: float
    shards: tuple

    def cue(self, i):
        """When the scene's `say` row `i` starts, in seconds from the scene's start."""
        return self.shards[i].start


class Row(NamedTuple):
    scene: str
    kind: str
    seconds: float | None
    text: str


def rows(path=SCRIPT):
    """The table's rows, in order."""
    out = []
    for line in path.read_text().splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")] if line.startswith("|") else []
        if len(cells) < 4 or cells[1] in ("kind", "") or set(cells[1]) <= {"-"}:
            continue
        scene, kind, seconds, text = cells[0], cells[1], cells[2], "|".join(cells[3:]).strip()
        if kind not in ("written", "say", "pause"):
            raise ValueError(f"{path.name}: unknown kind {kind!r} in row: {line}")
        out.append(Row(scene, kind, float(seconds) if seconds else None, text))
    return out


def timeline(path=SCRIPT):
    """The scenes of the script, timed by its rows."""
    scenes, start = [], 0.0
    for row in rows(path):
        if not scenes or scenes[-1]["name"] != row.scene:
            if scenes:
                start += scenes[-1]["t"]
            scenes.append({"name": row.scene, "written": "", "start": start, "t": 0.0, "shards": []})
        scene = scenes[-1]
        if row.kind == "written":
            scene["written"] = row.text
        elif row.kind == "pause":
            scene["t"] += row.seconds or 0.0
        else:
            seconds = row.seconds if row.seconds is not None else len(row.text.split()) / WORDS_PER_SECOND
            scene["shards"].append(Shard(row.text, scene["t"], seconds))
            scene["t"] += seconds
    return tuple(
        Scene(s["name"], s["written"], s["start"], s["t"], tuple(s["shards"])) for s in scenes
    )


def timings(scenes):
    """Every `say` row with its start and end, in video time, as a readable listing."""
    lines = []
    for scene in scenes:
        lines.append(f"{_clock(scene.start)}  {scene.name}  ({scene.duration:.1f} s)")
        for i, shard in enumerate(scene.shards):
            t0 = scene.start + shard.start
            lines.append(f"  {_clock(t0)}-{_clock(t0 + shard.duration)}  say {i}  {shard.text}")
    return "\n".join(lines)


def _clock(seconds):
    return f"{int(seconds) // 60}:{seconds % 60:04.1f}"


def _words(text):
    return [w for w in re.sub(r"[^\w\s'-]", " ", text.lower()).split() if w]


def captions(scenes):
    """SRT captions: a cue per sentence of the written form, timed by its `say` rows.

    A scene whose `say` rows don't say the same words as its written form gets a cue
    per `say` row instead.
    """
    cues = []
    for scene in scenes:
        # When each spoken word starts and ends, in video seconds
        times = []
        for shard in scene.shards:
            n = len(_words(shard.text))
            for k in range(n):
                t0 = scene.start + shard.start + shard.duration * k / n
                times.append((t0, t0 + shard.duration / n))
        sentences = re.split(r"(?<=[.!?:])\s+", scene.written)
        if scene.written and sum(len(_words(s)) for s in sentences) == len(times):
            i = 0
            for sentence in sentences:
                n = len(_words(sentence))
                cues.append((times[i][0], times[i + n - 1][1], sentence))
                i += n
        else:
            for shard in scene.shards:
                t0 = scene.start + shard.start
                cues.append((t0, t0 + shard.duration, shard.text))
    return "\n".join(
        f"{i}\n{_srt_time(t0)} --> {_srt_time(t1)}\n{text}\n"
        for i, (t0, t1, text) in enumerate(cues, start=1)
    )


def _srt_time(seconds):
    ms = round(seconds * 1000)
    return f"{ms // 3_600_000:02d}:{ms // 60_000 % 60:02d}:{ms // 1000 % 60:02d},{ms % 1000:03d}"
