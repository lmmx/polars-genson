"""Re-encode a video to fit a file size limit, e.g. Discord's.

    python video/shrink.py                                    # the voiced explainer, 19 MB
    python video/shrink.py --input out/json_to_map_x.mp4      # the X cut instead
    python video/shrink.py --size 9 --height 540              # a 10 MB limit
    python video/shrink.py --keep-grain                       # if smoothing shows banding

The size is hit by working out the bitrate it allows (the size over the video's length,
less the audio's share) and encoding in two passes: the first measures the video, the
second spends the bits where they're needed. The film grain is smoothed out first
(`hqdn3d`): it is noise, the most expensive thing to encode, and at a low bitrate x264
would spend most of the bits on it and smear it anyway.
"""

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path

OUT = Path(__file__).parent / "out"
DENOISE = "hqdn3d=3:3:6:6"


def duration(path):
    """The video's length in seconds, from ffprobe."""
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
        check=True, capture_output=True, text=True,
    )  # fmt: skip
    return float(probe.stdout.strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--input", type=Path, default=OUT / "json_to_map_voiced.mp4", help="default: %(default)s")
    parser.add_argument("--output", type=Path, help="default: the input's name with _small")
    parser.add_argument("--size", type=float, default=19.0, help="target size in MB (default: %(default)s)")
    parser.add_argument("--height", type=int, default=720, help="output height in px (default: %(default)s)")
    parser.add_argument("--audio", type=int, default=96, help="audio bitrate in kbit/s (default: %(default)s)")
    parser.add_argument("--keep-grain", action="store_true", help="don't smooth out the grain")
    args = parser.parse_args()

    for tool in ("ffmpeg", "ffprobe"):
        if shutil.which(tool) is None:
            raise SystemExit(f"{tool} isn't on the PATH")
    if not args.input.exists():
        raise SystemExit(f"no such file: {args.input}")
    output = args.output or args.input.with_name(f"{args.input.stem}_small.mp4")

    seconds = duration(args.input)
    total = args.size * 8_000 / seconds  # kbit/s, with 1 MB = 1,000,000 bytes
    video = int(total * 0.97 - args.audio)  # 3% for the container
    if video < 200:
        raise SystemExit(f"{args.size} MB over {seconds:.0f} s leaves {video} kbit/s for video: too little")
    print(f"{seconds:.1f} s at {video}k video + {args.audio}k audio -> about {args.size} MB")

    vf = f"scale=-2:{args.height}:flags=lanczos" + ("" if args.keep_grain else f",{DENOISE}")
    common = ["-vf", vf, "-c:v", "libx264", "-preset", "slow", "-b:v", f"{video}k"]
    with tempfile.TemporaryDirectory() as tmp:
        log = str(Path(tmp) / "pass")
        first = [
            "ffmpeg", "-y", "-loglevel", "error", "-i", str(args.input), *common,
            "-pass", "1", "-passlogfile", log, "-an", "-f", "null", "-",
        ]
        second = [
            "ffmpeg", "-y", "-loglevel", "error", "-i", str(args.input), *common,
            "-pass", "2", "-passlogfile", log,
            "-profile:v", "high", "-level:v", "4.1", "-g", "60", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", f"{args.audio}k", "-movflags", "+faststart",
            str(output),
        ]  # fmt: skip
        print("pass 1/2")
        subprocess.run(first, check=True)
        print("pass 2/2")
        subprocess.run(second, check=True)
    print(f"{output} ({output.stat().st_size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
