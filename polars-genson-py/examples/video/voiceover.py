"""The voiceover, and the scene timings that follow from it.

Each scene lasts as long as its line takes to say at `WORDS_PER_SECOND`, plus a pause
before and after, so changing a line here re-times both json_to_map_video.py and
teleprompter.py.
"""

import math

WORDS_PER_SECOND = 2.0
LEAD_IN, LEAD_OUT = 0.6, 0.8  # silent seconds at the start and end of each scene
MIN_SECONDS = 4.0  # long enough for a scene's visuals to settle

LINES = (
    ("Title", "Polars 2.0 introduces a new Map data type for DataFrames."),
    ("Records", "Some JSON objects are records. They have the same keys in every row, and every key has a corresponding column."),
    ("Maps", "Other JSON objects are maps. Their keys are data, like these language codes, and they change from row to row."),
    ("As a struct", "If you read these as a struct, you get a column for every language, and most of the values are null."),
    ("As a map", "If you read them as a map instead, there are no extra nulls: each row contains only its own languages."),
    ("genson", "polars-genson works out which objects are maps. If an object has more distinct keys than a threshold, it becomes a map."),
    ("Map functions", "Then you can use Polars' map functions on the column, for example to get a key or count the entries."),
    ("End card", "To try it out, pip install polars-genson."),
)


def timeline():
    """(scene, start, duration, line) for each scene, durations rounded up to 0.5 s."""
    start, scenes = 0.0, []
    for scene, line in LINES:
        speaking = len(line.split()) / WORDS_PER_SECOND
        duration = max(MIN_SECONDS, math.ceil((LEAD_IN + speaking + LEAD_OUT) * 2) / 2)
        scenes.append((scene, start, duration, line))
        start += duration
    return tuple(scenes)
