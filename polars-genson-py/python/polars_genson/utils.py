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
    """Split a version string like "0.20.16" into a tuple of ints, e.g. (0, 20, 16)."""
    if isinstance(version, str):
        version = version.split(".")
    return tuple(int(re.sub(r"\D", "", str(v))) for v in version)
