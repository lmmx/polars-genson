"""The voiceover (voiceover.txt), and the timings of both videos that follow from it.

Each shard lasts its words divided by the pace (words per second), unless it gives a
fixed length; each scene lasts as long as its shards, plus pauses. So editing
voiceover.txt, or rendering at another `--pace`, re-times both videos together.
"""

import math
import re
from pathlib import Path
from typing import NamedTuple

TRANSCRIPT = Path(__file__).with_name("voiceover.txt")
WORDS_PER_SECOND = 2.7  # the default pace
LEAD_IN, GAP, LEAD_OUT = 0.5, 0.25, 0.8  # seconds before, between and after shards
MIN_SECONDS = 4.0  # long enough for a scene's visuals to settle

FIXED = re.compile(r"^(?P<text>.*?)\s*\[(?P<seconds>\d+(?:\.\d+)?)s\]$")


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
        """When shard `i` starts, in seconds from the start of the scene."""
        return self.shards[i].start


def parse(path=TRANSCRIPT):
    """[(name, written, [(shard text, fixed seconds or None)])] from the transcript."""
    scenes = []
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if line.startswith("## "):
            scenes.append((line[3:].strip(), [], []))
        elif not line or line.startswith("#"):
            continue
        elif line.startswith("- "):
            m = FIXED.match(line[2:])
            shard = (m["text"], float(m["seconds"])) if m else (line[2:], None)
            scenes[-1][2].append(shard)
        else:
            scenes[-1][1].append(line)
    return [(name, " ".join(written), shards) for name, written, shards in scenes]


def timeline(words_per_second=WORDS_PER_SECOND, path=TRANSCRIPT):
    """The scenes of the transcript, timed at the given pace."""
    start, scenes = 0.0, []
    for name, written, shards in parse(path):
        t, timed = LEAD_IN, []
        for text, fixed in shards:
            duration = fixed if fixed is not None else len(text.split()) / words_per_second
            timed.append(Shard(text, t, duration))
            t += duration + GAP
        duration = max(MIN_SECONDS, math.ceil((t - GAP + LEAD_OUT) * 10) / 10)
        scenes.append(Scene(name, written, start, duration, tuple(timed)))
        start += duration
    return tuple(scenes)


def _words(text):
    return [w for w in re.sub(r"[^\w\s'-]", " ", text.lower()).split() if w]


def captions(scenes):
    """SRT captions: a cue per sentence of the written form, timed by its shards.

    A scene whose shards don't say the same words as its written form gets a cue per
    shard instead.
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
        if sum(len(_words(s)) for s in sentences) == len(times):
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
