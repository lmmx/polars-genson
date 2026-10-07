"""Render the json_to_map explainer or its teleprompter, timed by the voiceover.

    uv run --group video video/render.py explainer              # video/json_to_map.mp4
    uv run --group video video/render.py teleprompter           # video/teleprompter.mp4
    uv run --group video video/render.py teleprompter --pace 3  # words per second
    uv run --group video video/render.py explainer --preview    # PNG frames in video/preview

Both videos take their timings from voiceover.py at the given pace, so render them at
the same pace for a recording made with the teleprompter to line up with the explainer.
"""

import argparse
from pathlib import Path

from fframes.compose import RenderOptions

import json_to_map_video
import teleprompter
from voiceover import WORDS_PER_SECOND, timeline

HERE = Path(__file__).parent
VIDEOS = {
    "explainer": (json_to_map_video, "json_to_map.mp4"),
    "teleprompter": (teleprompter, "teleprompter.mp4"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("video", choices=VIDEOS)
    parser.add_argument(
        "--pace",
        type=float,
        default=WORDS_PER_SECOND,
        help="words per second of the voiceover (default: %(default)s)",
    )
    parser.add_argument("--preview", action="store_true", help="write PNG frames only")
    args = parser.parse_args()

    module, filename = VIDEOS[args.video]
    scenes = timeline(args.pace)
    end = scenes[-1][1] + scenes[-1][2]
    print(f"{args.video} at {args.pace} words/s: {end:.1f} s of scenes")
    compiled = module.build(scenes).compile()
    if args.preview:
        out = HERE / "preview"
        out.mkdir(exist_ok=True)
        for seconds in module.preview_times(scenes):
            path = out / f"{args.video}_{seconds:05.1f}s.png"
            compiled.save_png(str(path), index=int(seconds * module.FPS))
            print(path)
    else:
        path = HERE / filename
        compiled.render(str(path), options=RenderOptions(bitrate=8_000_000))
        print(path)


if __name__ == "__main__":
    main()
