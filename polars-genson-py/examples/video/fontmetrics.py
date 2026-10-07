"""Text widths from a TrueType font's own metrics, for laying out proportional text.

Reads each character's advance width from the font's `cmap` and `hmtx` tables, so a
line of text can be centred exactly. Kerning is not applied.
"""

import struct
from functools import cache
from pathlib import Path


@cache
def _metrics(path):
    """(units per em, {codepoint: advance width}) from the font file at `path`."""
    data = Path(path).read_bytes()
    num_tables = struct.unpack_from(">H", data, 4)[0]
    tables = {}
    for i in range(num_tables):
        tag, _, offset, _ = struct.unpack_from(">4sIII", data, 12 + 16 * i)
        tables[tag.decode()] = offset
    units = struct.unpack_from(">H", data, tables["head"] + 18)[0]
    num_metrics = struct.unpack_from(">H", data, tables["hhea"] + 34)[0]
    advances = struct.unpack_from(f">{num_metrics * 2}H", data, tables["hmtx"])[::2]

    # cmap: the Unicode BMP subtable (format 4)
    cmap = tables["cmap"]
    glyphs = {}
    for i in range(struct.unpack_from(">H", data, cmap + 2)[0]):
        platform, encoding, offset = struct.unpack_from(">HHI", data, cmap + 4 + 8 * i)
        sub = cmap + offset
        if struct.unpack_from(">H", data, sub)[0] != 4 or (platform, encoding) not in (
            (3, 1),
            (0, 3),
        ):
            continue
        segs = struct.unpack_from(">H", data, sub + 6)[0] // 2
        ends = struct.unpack_from(f">{segs}H", data, sub + 14)
        starts = struct.unpack_from(f">{segs}H", data, sub + 16 + 2 * segs)
        deltas = struct.unpack_from(f">{segs}h", data, sub + 16 + 4 * segs)
        ranges_at = sub + 16 + 6 * segs
        range_offsets = struct.unpack_from(f">{segs}H", data, ranges_at)
        for s, (start, end, delta, ro) in enumerate(
            zip(starts, ends, deltas, range_offsets)
        ):
            for code in range(start, min(end, 0xFFFE) + 1):
                if ro == 0:
                    glyph = (code + delta) & 0xFFFF
                else:
                    at = ranges_at + 2 * s + ro + 2 * (code - start)
                    glyph = struct.unpack_from(">H", data, at)[0]
                    glyph = (glyph + delta) & 0xFFFF if glyph else 0
                glyphs[code] = glyph
        break
    widths = {code: advances[min(g, num_metrics - 1)] for code, g in glyphs.items()}
    return units, widths


def width(text, path, size):
    """The advance width of `text` set in the font at `path`, at `size` px."""
    units, widths = _metrics(str(path))
    return sum(widths.get(ord(c), units // 2) for c in text) * size / units
