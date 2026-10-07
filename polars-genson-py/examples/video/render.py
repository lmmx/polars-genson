"""Render the json_to_map explainer, its teleprompter and its captions from script.md.

    uv run --group video video/render.py                       # all three, into video/out
    uv run --group video video/render.py explainer             # out/json_to_map.mp4
    uv run --group video video/render.py teleprompter          # out/teleprompter.mp4
    uv run --group video video/render.py captions              # out/json_to_map.srt
    uv run --group video video/render.py explainer --preview   # PNG frames in out/preview
    uv run --group video video/render.py timings               # when each line starts

All three are timed from script.md, so a recording made with the teleprompter lines up
with the explainer rendered from the same script.
"""

import argparse
from pathlib import Path

from voiceover import captions, timeline, timings

OUT = Path(__file__).parent / "out"
OUTPUTS = {
    "explainer": "json_to_map.mp4",
    "teleprompter": "teleprompter.mp4",
    "captions": "json_to_map.srt",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("output", nargs="?", default="all", choices=[*OUTPUTS, "all", "timings"])
    parser.add_argument("--preview", action="store_true", help="write PNG frames, not videos")
    args = parser.parse_args()

    scenes = timeline()
    if args.output == "timings":
        print(timings(scenes))
        return
    end = scenes[-1].start + scenes[-1].duration
    print(f"{len(scenes)} scenes, {end:.1f} s")
    OUT.mkdir(exist_ok=True)
    for output in OUTPUTS if args.output == "all" else [args.output]:
        render(output, scenes, preview=args.preview)


def render(output, scenes, *, preview=False):
    """Write one output (or, with `preview`, a video's PNG frames) into OUT."""
    path = OUT / OUTPUTS[output]
    if output == "captions":
        path.write_text(captions(scenes))
        print(path)
        return

    # Imported here, so captions and timings don't need fframes
    from fframes.compose import RenderOptions

    import json_to_map_video
    import teleprompter

    module = json_to_map_video if output == "explainer" else teleprompter
    compiled = module.build(scenes).compile()
    if preview:
        frames = OUT / "preview"
        frames.mkdir(exist_ok=True)
        for seconds in module.preview_times(scenes):
            frame = frames / f"{output}_{seconds:05.1f}s.png"
            compiled.save_png(str(frame), index=int(seconds * module.FPS))
            print(frame)
    else:
        compiled.render(str(path), options=RenderOptions(bitrate=8_000_000))
        print(path)


if __name__ == "__main__":
    main()
