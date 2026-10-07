"""Render the json_to_map explainer, its teleprompter or its captions, from voiceover.txt.

    uv run --group video video/render.py                        # all three
    uv run --group video video/render.py explainer              # video/json_to_map.mp4
    uv run --group video video/render.py teleprompter           # video/teleprompter.mp4
    uv run --group video video/render.py captions               # video/json_to_map.srt
    uv run --group video video/render.py teleprompter --pace 3  # words per second
    uv run --group video video/render.py explainer --preview    # PNG frames in video/preview

All three are timed from voiceover.txt at the given pace, so use the same pace for each:
a recording made with the teleprompter then lines up with the explainer.
"""

import argparse
from pathlib import Path

from voiceover import WORDS_PER_SECOND, captions, timeline

HERE = Path(__file__).parent
OUTPUTS = {
    "explainer": "json_to_map.mp4",
    "teleprompter": "teleprompter.mp4",
    "captions": "json_to_map.srt",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("output", nargs="?", default="all", choices=[*OUTPUTS, "all"])
    parser.add_argument(
        "--pace",
        type=float,
        default=WORDS_PER_SECOND,
        help="words per second of the voiceover (default: %(default)s)",
    )
    parser.add_argument("--preview", action="store_true", help="write PNG frames only")
    args = parser.parse_args()

    scenes = timeline(args.pace)
    end = scenes[-1].start + scenes[-1].duration
    print(f"{len(scenes)} scenes at {args.pace} words/s: {end:.1f} s")
    outputs = OUTPUTS if args.output == "all" else [args.output]
    for output in outputs:
        render(output, scenes, preview=args.preview)


def render(output, scenes, *, preview=False):
    """Write one output (or, with `preview`, a video's PNG frames)."""
    path = HERE / OUTPUTS[output]
    if output == "captions":
        path.write_text(captions(scenes))
        print(path)
        return

    # Imported here, so captions don't need fframes
    from fframes.compose import RenderOptions

    import json_to_map_video
    import teleprompter

    module = json_to_map_video if output == "explainer" else teleprompter
    compiled = module.build(scenes).compile()
    if preview:
        out = HERE / "preview"
        out.mkdir(exist_ok=True)
        for seconds in module.preview_times(scenes):
            frame = out / f"{output}_{seconds:05.1f}s.png"
            compiled.save_png(str(frame), index=int(seconds * module.FPS))
            print(frame)
    else:
        compiled.render(str(path), options=RenderOptions(bitrate=8_000_000))
        print(path)


if __name__ == "__main__":
    main()
