"""Add a voiceover recording to the rendered explainer, with ffmpeg.

    python video/add_audio.py voiceover.m4a
    python video/add_audio.py voiceover.m4a --offset 4.2   # skip 4.2 s of the recording

The recording is assumed to start with the teleprompter's countdown, so its first
`--offset` seconds (the countdown, 3 s, by default) are skipped to line it up with the
explainer's 0:00. If you started recording before the teleprompter started playing,
add that lead to the offset. The video is copied as it is; the audio is encoded as AAC,
and padded with silence or cut to the video's length.
"""

import argparse
import shutil
import subprocess
from pathlib import Path

OUT = Path(__file__).parent / "out"
COUNTDOWN = 3.0  # teleprompter.COUNTDOWN: seconds of countdown before the voiceover's 0:00


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("audio", type=Path, help="the voiceover recording")
    parser.add_argument("--video", type=Path, default=OUT / "json_to_map.mp4", help="default: %(default)s")
    parser.add_argument("--output", type=Path, default=OUT / "json_to_map_voiced.mp4", help="default: %(default)s")
    parser.add_argument("--offset", type=float, default=COUNTDOWN, help="seconds of the recording to skip (default: %(default)s)")
    args = parser.parse_args()

    if shutil.which("ffmpeg") is None:
        raise SystemExit("ffmpeg isn't on the PATH")
    for path in (args.audio, args.video):
        if not path.exists():
            raise SystemExit(f"no such file: {path}")
    command = [
        "ffmpeg", "-y", "-loglevel", "error",
        "-i", str(args.video),
        "-ss", str(args.offset), "-i", str(args.audio),  # skip the countdown
        "-map", "0:v", "-map", "1:a",
        "-c:v", "copy",
        "-af", "apad",  # pad the voice with silence if it ends first...
        "-c:a", "aac", "-b:a", "192k",
        "-shortest",  # ...and end with the video
        "-movflags", "+faststart",
        str(args.output),
    ]  # fmt: skip
    subprocess.run(command, check=True)
    print(args.output)


if __name__ == "__main__":
    main()
