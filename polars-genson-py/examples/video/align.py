"""Time script.md from a voiceover recording, so the videos follow how it was spoken.

    uv run --with faster-whisper python video/align.py out/json_to_map_voiced.mp4

Transcribes the recording locally (faster-whisper, word timestamps), matches its words
to the script's `say` rows, and rewrites the rows' seconds and the pauses between them
so each row starts and ends where it was said. The recording is taken to be placed as
`add_audio.py` places it: its times are the explainer's. The first pause (before the
first word) is kept, so `add_audio.py` still places the recording the same way.

Re-render the explainer afterwards (`render.py explainer`), then `add_audio.py`.
"""

import argparse
import difflib
import re
import subprocess
from pathlib import Path

from voiceover import SCRIPT, rows

MIN_HOLD = (
    1.0  # seconds a scene's last visuals stay on screen at least, before the next
)
LEAD_IN = 0.8  # seconds before a scene's first word, when the gap allows


def tokens(text):
    """Lowercase words and numbers, splitting on anything else (so "map.get" is 2)."""
    return re.findall(r"[a-z0-9]+", text.lower().replace("'", ""))


def transcribe(path, model="small.en"):
    """[(token, start, end)] for each spoken word, from faster-whisper.

    The audio is decoded with the system ffmpeg (16 kHz mono, as Whisper takes it) and
    passed as samples, so faster-whisper's own decoder (PyAV) isn't used: its call
    breaks with newer PyAV releases.
    """
    import numpy as np
    from faster_whisper import WhisperModel

    decoded = subprocess.run(
        [
            "ffmpeg",
            "-loglevel",
            "error",
            "-i",
            str(path),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-f",
            "f32le",
            "-",
        ],
        check=True,
        capture_output=True,
    ).stdout
    audio = np.frombuffer(decoded, dtype=np.float32)
    segments, _ = WhisperModel(model, device="cpu", compute_type="int8").transcribe(
        audio, word_timestamps=True, beam_size=5
    )
    out = []
    for segment in segments:
        for word in segment.words:
            for token in tokens(word.word):
                out.append((token, word.start, word.end))
    return out


def say_times(script_rows, spoken):
    """(start, end) of each `say` row, from where its words were spoken."""
    says = [r for r in script_rows if r.kind == "say"]
    script_tokens, owner = [], []
    for k, row in enumerate(says):
        for token in tokens(row.text):
            script_tokens.append(token)
            owner.append(k)
    heard = [t for t, _, _ in spoken]
    # For each script token, the index of the spoken word it lines up with
    at = [None] * len(script_tokens)
    matcher = difflib.SequenceMatcher(a=script_tokens, b=heard, autojunk=False)
    for op, i1, i2, j1, j2 in matcher.get_opcodes():
        for i in range(i1, i2):
            if op == "equal":
                at[i] = j1 + (i - i1)
            elif (
                op == "replace"
            ):  # misheard: spread the script's words over the heard ones
                at[i] = min(j2 - 1, j1 + (i - i1) * (j2 - j1) // (i2 - i1))
            elif op == "delete":  # not heard: at the next heard word
                at[i] = min(j1, len(heard) - 1)
    times = []
    for k in range(len(says)):
        idx = [at[i] for i, o in enumerate(owner) if o == k]
        times.append((spoken[idx[0]][1], spoken[idx[-1]][2]))
    return says, times


def retime(script_rows, says, times):
    """New seconds for every `say` and `pause` row, as {row index: seconds}.

    Walks each scene's rows with a time cursor: a `say` row lasts until its last word
    ends, a pause until the next word starts. The silence between scenes is split into
    the outgoing scene's hold (at least MIN_HOLD) and the next scene's lead-in.
    """
    when = {id(r): t for r, t in zip(says, times)}
    scenes = []
    for i, r in enumerate(script_rows):
        if not scenes or scenes[-1][0] != r.scene:
            scenes.append((r.scene, []))
        scenes[-1][1].append((i, r))
    # When each scene starts: its first word, less its lead-in
    starts = [0.0]
    for (_, prev), (_, this) in zip(scenes, scenes[1:]):
        last_end = when[id([r for _, r in prev if r.kind == "say"][-1])][1]
        first = when[id(next(r for _, r in this if r.kind == "say"))][0]
        lead = min(LEAD_IN, max(0.0, first - last_end - MIN_HOLD))
        starts.append(first - lead)
    seconds = {}
    for n, (_, scene_rows) in enumerate(scenes):
        cursor = starts[n]
        scene_says = [r for _, r in scene_rows if r.kind == "say"]
        for i, r in scene_rows:
            if r.kind == "say":
                end = when[id(r)][1]
                seconds[i] = max(0.1, end - cursor)
                cursor += seconds[i]
            elif r.kind == "pause":
                upcoming = [
                    when[id(x)][0] for x in scene_says if when[id(x)][1] > cursor + 1e-6
                ]
                if (
                    n == 0 and cursor == 0.0
                ):  # the opening pause stays, for add_audio.py
                    target = cursor + r.seconds
                elif upcoming:
                    target = upcoming[0]
                elif n + 1 < len(starts):
                    target = starts[n + 1]
                else:  # the last scene's hold stays
                    target = cursor + r.seconds
                seconds[i] = max(0.0, target - cursor)
                cursor += seconds[i]
    return seconds


def write(seconds, path=SCRIPT):
    """Rewrite the seconds column of the table's rows in `path`."""
    lines, n = path.read_text().splitlines(keepends=True), -1
    for k, line in enumerate(lines):
        cells = line.split("|")
        if (
            not line.startswith("|")
            or len(cells) < 5
            or cells[2].strip() not in ("say", "pause", "written")
        ):
            continue
        n += 1
        if n in seconds:
            cells[3] = f" {seconds[n]:>7.2f} "
            lines[k] = "|".join(cells)
    path.write_text("".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "recording",
        type=Path,
        help="the voiced explainer, or the audio as placed in it",
    )
    args = parser.parse_args()
    script_rows = rows()
    spoken = transcribe(args.recording)
    says, times = say_times(script_rows, spoken)
    write(retime(script_rows, says, times))
    print(
        f"re-timed {len(says)} lines in {SCRIPT.name} from {len(spoken)} spoken words"
    )


if __name__ == "__main__":
    main()
