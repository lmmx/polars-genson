"""Add a voiceover recording to the rendered explainer, with ffmpeg.

    python video/add_audio.py voiceover.m4a
    python video/add_audio.py voiceover.m4a --start 1.0   # first word 1.0 s in

The recording should be trimmed to start at its first word. It's placed so that word
falls when the first `say` row of script.md starts (`render.py timings` lists it), or
at `--start`. The video is copied as it is; the audio is encoded as AAC, and padded
with silence or cut to the video's length.
"""

import argparse
import shutil
import subprocess
from pathlib import Path

from voiceover import timeline

OUT = Path(__file__).parent / "out"


def first_line():
    """When the script's first `say` row starts, in seconds from the video's start."""
    scene = timeline()[0]
    return scene.start + scene.shards[0].start


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("audio", type=Path, help="the voiceover, trimmed to its first word")
    parser.add_argument("--start", type=float, default=first_line(), help="seconds into the video of the first word (default: %(default)s)")
    parser.add_argument("--video", type=Path, default=OUT / "json_to_map.mp4", help="default: %(default)s")
    parser.add_argument("--output", type=Path, default=OUT / "json_to_map_voiced.mp4", help="default: %(default)s")
    args = parser.parse_args()

    if shutil.which("ffmpeg") is None:
        raise SystemExit("ffmpeg isn't on the PATH")
    for path in (args.audio, args.video):
        if not path.exists():
            raise SystemExit(f"no such file: {path}")
    delay_ms = round(args.start * 1000)
    command = [
        "ffmpeg", "-y", "-loglevel", "error",
        "-i", str(args.video),
        "-i", str(args.audio),
        "-map", "0:v", "-map", "1:a",
        "-c:v", "copy",
        # Delay the voice to its first line, and pad it with silence if it ends first...
        "-af", f"adelay={delay_ms}:all=1,apad",
        "-c:a", "aac", "-b:a", "192k",
        "-shortest",  # ...ending with the video
        "-movflags", "+faststart",
        str(args.output),
    ]  # fmt: skip
    subprocess.run(command, check=True)
    print(f"{args.output} (voice from {args.start:.2f} s)")


if __name__ == "__main__":
    main()
