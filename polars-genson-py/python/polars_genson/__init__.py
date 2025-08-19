"""A Polars plugin for JSON schema inference from string columns using genson-rs."""

from __future__ import annotations

import inspect
from pathlib import Path

import orjson
import polars as pl
from polars.api import register_dataframe_namespace
from polars.plugins import register_plugin_function

from .utils import parse_into_expr, parse_version  # noqa: F401

# Determine the correct plugin path
if parse_version(pl.__version__) < parse_version("0.20.16"):
    from polars.utils.udfs import _get_shared_lib_location

    lib: str | Path = _get_shared_lib_location(__file__)
else:
    lib = Path(__file__).parent

__all__ = ["infer_json_schema"]


def plug(expr: pl.Expr, **kwargs) -> pl.Expr:
    """Wrap Polars' `register_plugin_function` helper to always pass the same `lib`."""
    func_name = inspect.stack()[1].function
    return register_plugin_function(
        plugin_path=lib,
        function_name=func_name,
        args=expr,
        is_elementwise=False,  # This is an aggregation across rows
        kwargs=kwargs,
    )


def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "AUTO",
    merge_schemas: bool = True,
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
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
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
        "merge_schemas": merge_schemas,
        "debug": debug,
    }
    if schema_uri is not None:
        kwargs["schema_uri"] = schema_uri

    return plug(expr, **kwargs)


def infer_polars_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
) -> pl.Expr:
    """Infer Polars schema from a string column containing JSON data.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column containing JSON data
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
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
        "merge_schemas": merge_schemas,
        "debug": debug,
    }

    return plug(expr, **kwargs)


@register_dataframe_namespace("genson")
class GensonNamespace:
    """Namespace for JSON schema inference operations."""

    def __init__(self, df: pl.DataFrame):
        self._df = df

    def infer_polars_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        merge_schemas: bool = True,
        debug: bool = False,
    ) -> pl.Schema:
        # ) -> pl.Schema | list[pl.Schema]:
        """Infer Polars schema from a string column containing JSON data.

        Parameters
        ----------
        column : str
            Name of the column containing JSON strings
        ignore_outer_array : bool, default True
            Whether to treat top-level arrays as streams of objects
        ndjson : bool, default False
            Whether to treat input as newline-delimited JSON
        merge_schemas : bool, default True
            Whether to merge schemas from all rows (True) or return individual schemas (False)
        debug : bool, default False
            Whether to print debug information

        Returns:
        -------
        pl.Schema | list[pl.Schema]
            The inferred schema (if merge_schemas=True) or list of schemas (if merge_schemas=False)
        """
        if not merge_schemas:
            raise NotImplementedError("Only merge schemas is implemented")
        result = self._df.select(
            infer_polars_schema(
                pl.col(column),
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                merge_schemas=merge_schemas,
                debug=debug,
            ).first()
        )

        # Extract the schema from the first column, which is the struct
        schema_info = result.to_series().item()
        schema = pl.Schema(
            {field["name"]: getattr(pl, field["dtype"]) for field in schema_info}
        )
        return schema

    def infer_json_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        schema_uri: str | None = "AUTO",
        merge_schemas: bool = True,
        debug: bool = False,
    ) -> dict | list[dict]:
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
        merge_schemas : bool, default True
            Whether to merge schemas from all rows (True) or return individual schemas (False)
        debug : bool, default False
            Whether to print debug information

        Returns:
        -------
        dict | list[dict]
            The inferred JSON schema as a dictionary (if merge_schemas=True) or
            list of schemas (if merge_schemas=False)
        """
        result = self._df.select(
            infer_json_schema(
                pl.col(column),
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                schema_uri=schema_uri,
                merge_schemas=merge_schemas,
                debug=debug,
            ).first()
        )

        # Extract the schema from the first column (whatever it's named)
        schema_json = result.to_series().item()
        if not isinstance(schema_json, str):
            raise ValueError(f"Expected string schema, got {type(schema_json)}")

        try:
            return orjson.loads(schema_json)
        except orjson.JSONDecodeError as e:
            raise ValueError(f"Failed to parse schema JSON: {e}") from e
