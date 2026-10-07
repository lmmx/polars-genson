"""Cut scenes out of the voiced explainer, e.g. to fit a platform's length limit.

    python video/cut.py                                   # drops Map keys and Map functions
    python video/cut.py --drop "Map keys" "Naming maps"   # drops these instead
    python video/cut.py --keep-all --output out/full.mp4  # just re-encodes

Scene times come from script.md (`render.py timings` lists them), so this cuts at the
scene boundaries, in the pauses where one scene fades into the next. The result is
re-encoded as H.264 at a constant 30 fps with AAC audio, which X (Twitter) expects.
X allows 2:20 on a free account; the length of the cut is checked against `--max`.
"""

import argparse
import shutil
import subprocess
from pathlib import Path

from voiceover import timeline

OUT = Path(__file__).parent / "out"
DROP = ("Map keys", "Map functions")


def segments(scenes, drop):
    """The (start, end) spans to keep, joining neighbouring kept scenes."""
    spans = []
    for scene in scenes:
        if scene.name in drop:
            continue
        start, end = scene.start, scene.start + scene.duration
        if spans and abs(spans[-1][1] - start) < 1e-6:
            spans[-1] = (spans[-1][0], end)
        else:
            spans.append((start, end))
    return spans


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--drop", nargs="+", default=list(DROP), metavar="SCENE", help="scenes to cut (default: %(default)s)")
    parser.add_argument("--keep-all", action="store_true", help="cut nothing")
    parser.add_argument("--input", type=Path, default=OUT / "json_to_map_voiced.mp4", help="default: %(default)s")
    parser.add_argument("--output", type=Path, default=OUT / "json_to_map_x.mp4", help="default: %(default)s")
    parser.add_argument("--max", type=float, default=140.0, help="longest allowed, in seconds (default: %(default)s)")
    args = parser.parse_args()

    scenes = timeline()
    drop = set() if args.keep_all else set(args.drop)
    unknown = drop - {s.name for s in scenes}
    if unknown:
        raise SystemExit(f"no such scene: {', '.join(sorted(unknown))} (see `render.py timings`)")
    spans = segments(scenes, drop)
    length = sum(end - start for start, end in spans)
    print(f"keeping {len(spans)} span(s), {length:.1f} s: " + ", ".join(f"{a:.1f}-{b:.1f}" for a, b in spans))
    if length > args.max:
        raise SystemExit(f"{length:.1f} s is over the {args.max:.0f} s limit: drop more scenes")
    if shutil.which("ffmpeg") is None:
        raise SystemExit("ffmpeg isn't on the PATH")

    parts, joins = [], ""
    for k, (start, end) in enumerate(spans):
        parts.append(f"[0:v]trim={start:.3f}:{end:.3f},setpts=PTS-STARTPTS[v{k}]")
        parts.append(f"[0:a]atrim={start:.3f}:{end:.3f},asetpts=PTS-STARTPTS[a{k}]")
        joins += f"[v{k}][a{k}]"
    graph = ";".join(parts) + f";{joins}concat=n={len(spans)}:v=1:a=1[v][a]"
    command = [
        "ffmpeg", "-y", "-loglevel", "error", "-i", str(args.input),
        "-filter_complex", graph, "-map", "[v]", "-map", "[a]",
        "-c:v", "libx264", "-preset", "slow", "-crf", "16", "-pix_fmt", "yuv420p", "-r", "30",
        "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart",
        str(args.output),
    ]  # fmt: skip
    subprocess.run(command, check=True)
    print(args.output)


if __name__ == "__main__":
    main()
