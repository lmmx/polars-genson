"""A Polars plugin for JSON schema inference from string columns using genson-rs."""

from __future__ import annotations

import inspect
from pathlib import Path

import polars as pl
from polars.api import register_dataframe_namespace
from polars.plugins import register_plugin_function

from .utils import parse_into_expr, parse_version  # noqa: F401

__all__ = ["infer_json_schema"]

lib = Path(__file__).parent


def plug(expr: pl.Expr, **kwargs) -> pl.Expr:
    """Wrap Polars' `register_plugin_function` helper to always pass the same `lib`.

    Always pass the same `lib` (the directory where _polars_genson.so/pyd lives).
    """
    func_name = inspect.stack()[1].function
    return register_plugin_function(
        plugin_path=lib,
        function_name=func_name,
        args=expr,
        is_elementwise=True,
        kwargs=kwargs,
    )


def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "AUTO",
    debug: bool = False,
) -> pl.Expr:
    """Infer JSON schema from a string column containing JSON data.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column containing JSON data
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    schema_uri : str or None, default "AUTO"
        Schema URI to use for the generated schema ("AUTO" for auto-detection)
    debug : bool, default False
        Whether to print debug information

    Returns:
    -------
    pl.Expr
        Expression representing the inferred JSON schema
    """
    kwargs = {
        "ignore_outer_array": ignore_outer_array,
        "ndjson": ndjson,
        "debug": debug,
    }
    if schema_uri is not None:
        kwargs["schema_uri"] = schema_uri

    return plug(expr, **kwargs)


@register_dataframe_namespace("genson")
class GensonNamespace:
    """Namespace for JSON schema inference operations."""

    def __init__(self, df: pl.DataFrame):
        self._df = df

    def infer_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        schema_uri: str | None = "AUTO",
        debug: bool = False,
    ) -> dict:
        """Infer JSON schema from a string column containing JSON data.

        Parameters
        ----------
        column : str
            Name of the column containing JSON strings
        ignore_outer_array : bool, default True
            Whether to treat top-level arrays as streams of objects
        ndjson : bool, default False
            Whether to treat input as newline-delimited JSON
        schema_uri : str or None, default "AUTO"
            Schema URI to use for the generated schema ("AUTO" for auto-detection)
        debug : bool, default False
            Whether to print debug information

        Returns:
        -------
        dict
            The inferred JSON schema as a dictionary
        """
        result = self._df.select(
            infer_json_schema(
                pl.col(column),
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                schema_uri=schema_uri,
                debug=debug,
            ).alias("schema")
        )

        # Extract the schema from the result
        schema_value = result.get_column("schema").item()
        return schema_value
