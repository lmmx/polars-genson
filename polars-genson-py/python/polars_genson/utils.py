"""Utility functions for polars-genson plugin."""

from __future__ import annotations

import re
from collections.abc import Sequence
from typing import TYPE_CHECKING

import polars as pl

if TYPE_CHECKING:
    from polars.type_aliases import IntoExpr, PolarsDataType


def parse_into_expr(
    expr: IntoExpr,
    *,
    str_as_lit: bool = False,
    list_as_lit: bool = True,
    dtype: PolarsDataType | None = None,
) -> pl.Expr:
    """Convert the user input into a polars.Expr.

    - If `expr` is already an `pl.Expr`, we return it as-is.
    - If `expr` is a string and `str_as_lit=False`, interpret as `pl.col(expr)`.
    - Otherwise, treat it as a literal (possibly typed by `dtype`).
    """
    if isinstance(expr, pl.Expr):
        return expr
    elif isinstance(expr, str) and not str_as_lit:
        return pl.col(expr)
    elif isinstance(expr, list) and not list_as_lit:
        return pl.lit(pl.Series(expr), dtype=dtype)
    else:
        return pl.lit(expr, dtype=dtype)


def parse_version(version: Sequence[str | int]) -> tuple[int, ...]:
    """Simple version parser; splits a version string like "0.20.16" into a tuple of ints.

    Takes a version string like "0.20.16" and converts it into a tuple of ints (0, 20, 16).
    """
    if isinstance(version, str):
        version = version.split(".")
    return tuple(int(re.sub(r"\D", "", str(v))) for v in version)


def format_time(minutes: int) -> str:
    """Convert minutes since midnight to "HH:MM" format.

    Args:
        minutes: Minutes since midnight

    Returns:
        Time string in "HH:MM" format
    """
    hours, mins = divmod(minutes, 60)
    return f"{hours:02d}:{mins:02d}"


def parse_constraint(constraint: str) -> tuple[str, int, str]:
    """Parse a constraint string into its components.

    Recognized formats:
    - "≥Xh apart"
    - "≥Xh before CATEGORY"
    - "≥Xh after CATEGORY"

    Args:
        constraint: Constraint string

    Returns:
        Tuple of (type, hours, reference)
    """
    apart_pattern = r"^≥(\d+)h\s+apart$"
    before_pattern = r"^≥(\d+)h\s+before\s+(.+)$"
    after_pattern = r"^≥(\d+)h\s+after\s+(.+)$"

    if match := re.match(apart_pattern, constraint):
        return "apart", int(match.group(1)), ""
    elif match := re.match(before_pattern, constraint):
        return "before", int(match.group(1)), match.group(2)
    elif match := re.match(after_pattern, constraint):
        return "after", int(match.group(1)), match.group(2)
    else:
        raise ValueError(f"Invalid constraint format: {constraint}")


def parse_window(window: str) -> dict[str, str | int]:
    """Parse a window string into its components.

    Recognized formats:
    - "HH:MM" (anchor)
    - "HH:MM-HH:MM" (range)

    Args:
        window: Window string

    Returns:
        Dictionary with window information
    """
    if "-" in window:
        start_str, end_str = window.split("-", 1)
        start_minutes = parse_time(start_str.strip())
        end_minutes = parse_time(end_str.strip())

        if end_minutes <= start_minutes:
            raise ValueError(f"Window end time must be after start time: {window}")

        return {
            "type": "range",
            "start": start_minutes,
            "end": end_minutes,
            "display": window,
        }
    else:
        minutes = parse_time(window.strip())
        return {
            "type": "anchor",
            "time": minutes,
            "display": window,
        }


def parse_time(time_str: str) -> int:
    """Convert "HH:MM" string to minutes since midnight.

    Args:
        time_str: Time string in "HH:MM" format

    Returns:
        Minutes since midnight (e.g., "08:30" -> 510)
    """
    match = re.match(r"^(\d{1,2}):(\d{2})$", time_str)
    if not match:
        raise ValueError(f"Invalid time format: {time_str}. Expected 'HH:MM'")

    hours, minutes = int(match.group(1)), int(match.group(2))
    if not (0 <= hours <= 23 and 0 <= minutes <= 59):
        raise ValueError(f"Time out of range: {time_str}")

    return hours * 60 + minutes
