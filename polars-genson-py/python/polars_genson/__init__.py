"""A Polars plugin for JSON schema inference from string columns using genson-rs."""

from __future__ import annotations

import inspect
from pathlib import Path
from typing import Literal

import orjson
import polars as pl
from polars.api import register_dataframe_namespace
from polars.plugins import register_plugin_function

from ._polars_genson import json_to_schema as _rust_json_to_schema
from ._polars_genson import schema_to_json as _rust_schema_to_json
from .dtypes import _parse_polars_dtype
from .utils import parse_into_expr, parse_version  # noqa: F401

# Determine the correct plugin path
if parse_version(pl.__version__) < parse_version("0.20.16"):
    from polars.utils.udfs import _get_shared_lib_location

    lib: str | Path = _get_shared_lib_location(__file__)
else:
    lib = Path(__file__).parent

__all__ = ["infer_json_schema", "json_to_schema", "schema_to_json"]


def json_to_schema(json_str: str) -> pl.Schema:
    """Convert a Polars schema to JSON string representation.

    Parameters
    ----------
    str
        JSON string to convert to Polars schema

    Returns:
    -------
    schema : pl.Schema
        The Polars schema representation of the JSON
    """
    df = _rust_json_to_schema(json_str)
    schema = df.schema
    return schema


def schema_to_json(schema: pl.Schema) -> str:
    """Convert a Polars schema to JSON string representation.

    Parameters
    ----------
    schema : pl.Schema
        The Polars schema to convert to JSON

    Returns:
    -------
    str
        JSON string representation of the schema
    """
    assert isinstance(schema, pl.Schema), (
        f"Expected Schema, got {type(schema)}: {schema}"
    )
    empty_df = schema.to_frame()
    return _rust_schema_to_json(empty_df)


def plug(expr: pl.Expr, changes_length: bool, **kwargs) -> pl.Expr:
    """Wrap Polars' `register_plugin_function` helper to always pass the same `lib`.

    Pass `changes_length` when using the `merge_schemas` (per-row) inference, as we only
    build a single schema in that case (so it'd be a waste to make more than one row).
    """
    func_name = inspect.stack()[1].function
    return register_plugin_function(
        plugin_path=lib,
        function_name=func_name,
        args=expr,
        is_elementwise=False,  # This is an aggregation across rows
        changes_length=changes_length,
        kwargs=kwargs,
    )


def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
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
    schema_uri : str or None, default "http://json-schema.org/schema#"
        Schema URI to use for the generated schema
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
    debug : bool, default False
        Whether to print debug information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count (preserves existing behavior).
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects. This avoids unification
        failures between scalars and objects. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    avro: bool, default False
        Whether to output an Avro schema instead of JSON schema.
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria

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
        "verbosity": verbosity,
        "map_threshold": map_threshold,
        "map_max_required_keys": map_max_required_keys,
        "unify_maps": unify_maps,
        "no_unify": list(no_unify) if no_unify else [],
        "wrap_scalars": wrap_scalars,
        "avro": avro,
        "wrap_root": wrap_root,
        "no_root_map": no_root_map,
    }
    if schema_uri is not None:
        kwargs["schema_uri"] = schema_uri
    if force_field_types is not None:
        kwargs["force_field_types"] = force_field_types

    return plug(expr, changes_length=merge_schemas, **kwargs)


def infer_polars_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
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
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects. This avoids unification
        failures between scalars and objects. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    avro: bool, default False
        Whether to read the input as an Avro schema instead of JSON schema.
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria

    Returns:
    -------
    pl.Expr
        Expression yielding the inferred Polars schema (as a struct of {name, dtype} fields).
    """
    kwargs = {
        "ignore_outer_array": ignore_outer_array,
        "ndjson": ndjson,
        "merge_schemas": merge_schemas,
        "debug": debug,
        "verbosity": verbosity,
        "map_threshold": map_threshold,
        "map_max_required_keys": map_max_required_keys,
        "unify_maps": unify_maps,
        "no_unify": list(no_unify) if no_unify else [],
        "wrap_scalars": wrap_scalars,
        "avro": avro,
        "wrap_root": wrap_root,
        "no_root_map": no_root_map,
    }
    if not merge_schemas:
        url = "https://github.com/lmmx/polars-genson/issues/37"
        raise NotImplementedError("Merge schemas for Polars schemas is TODO: see {url}")
    if force_field_types is not None:
        kwargs["force_field_types"] = force_field_types

    return plug(expr, changes_length=merge_schemas, **kwargs)


def normalise_json(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] = "kv",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
) -> pl.Expr:
    """Normalise a JSON string column against an inferred Avro schema.

    This performs schema inference once across all rows, then rewrites each row
    to conform to that schema. The output is a new column of JSON strings with
    consistent structure and datatypes.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column of JSON data.
    ignore_outer_array : bool, default True
        Treat a top-level JSON array as a stream of objects (like NDJSON).
    ndjson : bool, default False
        Treat input as newline-delimited JSON rather than a single JSON document.
    empty_as_null : bool, default True
        Convert empty arrays/maps into `null` to preserve row count when exploding.
        Disable with ``False`` to keep empty collections.
    coerce_strings : bool, default False
        If True, attempt to coerce string values into numeric/boolean types
        where the schema expects them. If False, unmatched strings become null.
    map_encoding : {"mapping", "entries", "kv"}, default "kv"
        Encoding to use for Avro maps:
        - "mapping": plain JSON object ({"en":"Hello"})
        - "entries": list of single-entry objects ([{"en":"Hello"}])
        - "kv":      list of {key,value} dicts ([{"key":"en","value":"Hello"}])
    map_threshold : int, default 20
        Maximum number of keys before an object is treated as a map
        (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference during schema
        inference. Objects with more required keys will be forced to Record type.
        If None, no gating based on required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Override the inferred type for specific fields. Keys are field names,
        values must be either ``"map"`` or ``"record"``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects. This avoids unification
        failures between scalars and objects. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    wrap_root : str | None, default None
        Wrap each JSON row under that key before normalisation.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria

    Returns:
    -------
    pl.Expr
        An expression producing a new string column, where each row is a
        normalised JSON object matching the inferred Avro schema.

    Examples:
    --------
    >>> df = pl.DataFrame({
    ...     "json_data": [
    ...         '{"id": "1", "labels": {}}',
    ...         '{"id": 2, "labels": {"en": "Hello"}}',
    ...     ]
    ... })
    >>> df.select(normalise_json(pl.col("json_data")))
    shape: (2, 1)
    ┌──────────────────────────────────────┐
    │ normalised                           │
    │ ---                                  │
    │ str                                  │
    ╞══════════════════════════════════════╡
    │ {"id": "1", "labels": null}          │
    │ {"id": "2", "labels": {"en":"Hello"}}│
    └──────────────────────────────────────┘
    """
    kwargs = {
        "ignore_outer_array": ignore_outer_array,
        "ndjson": ndjson,
        "empty_as_null": empty_as_null,
        "coerce_string": coerce_strings,
        "map_encoding": map_encoding,
        "map_threshold": map_threshold,
        "map_max_required_keys": map_max_required_keys,
        "unify_maps": unify_maps,
        "no_unify": list(no_unify) if no_unify else [],
        "wrap_scalars": wrap_scalars,
        "wrap_root": wrap_root,
        "no_root_map": no_root_map,
    }
    if force_field_types is not None:
        kwargs["force_field_types"] = force_field_types

    return plug(expr, changes_length=True, **kwargs)


@register_dataframe_namespace("genson")
class GensonNamespace:
    """Namespace for JSON schema inference operations."""

    def __init__(self, df: pl.DataFrame):
        self._df = df

    def schema_to_json(self) -> str:
        """Convert the DataFrame's schema to JSON string representation.

        Returns:
        -------
        str
            JSON string representation of the DataFrame's schema
        """
        return _rust_schema_to_json(self._df)

    def infer_polars_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        merge_schemas: bool = True,
        debug: bool = False,
        verbosity: Literal["Normal", "Verbose"] = "Normal",
        map_threshold: int = 20,
        map_max_required_keys: int | None = None,
        unify_maps: bool = False,
        no_unify: set[str] | None = None,
        force_field_types: dict[str, str] | None = None,
        wrap_scalars: bool = True,
        avro: bool = False,
        wrap_root: bool | str | None = None,
        no_root_map: bool = True,
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
        verbosity : str, default "Normal"
            Whether to print verbose debug information
        map_threshold : int, default 20
            Number of keys above which a heterogeneous object may be rewritten
            as a map (unless overridden).
        map_max_required_keys : int, optional
            Maximum number of required keys allowed for Map inference. Objects with more
            required keys will be forced to Record type. If None, no gating based on
            required key count.
        unify_maps : bool, default False
            Enable unification of compatible but non-homogeneous record schemas into maps.
            When True, record schemas with compatible field types can be merged into a single
            map schema with selective nullable fields.
        no_unify: set[str] | None, default None
            Prevent unification of keys under these field names with their sibling record fields.
        force_field_types : dict[str, str], optional
            Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
            Example: ``{"labels": "map", "claims": "record"}``.
        wrap_scalars : bool, default True
            Whether to promote scalar values into singleton objects when they appear
            in contexts where other rows provide objects. This avoids unification
            failures between scalars and objects. The promoted field name defaults
            to the parent key with a ``__{type}`` suffix, e.g. a string under
            ``"value"`` becomes ``{"value__string": "..."}``.
        avro : bool, default False
            Whether to infer using Avro schema semantics (unions, maps, nullability).
            By default (`False`), JSON Schema mode is used.
        wrap_root : str | bool | None, default None
            If a string, wrap each JSON row under that key before inference.
            If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
        no_root_map : bool, default True
            Prevent document root from becoming a map type, even if it meets map inference criteria

        Returns:
        -------
        pl.Schema | list[pl.Schema]
            The inferred schema (if merge_schemas=True) or list of schemas (if merge_schemas=False)
        """
        if not merge_schemas:
            raise NotImplementedError("Only merge schemas is implemented")
        fft = (
            {}
            if force_field_types is None
            else {"force_field_types": force_field_types}
        )
        wrap_root_field = column if wrap_root is True else wrap_root
        result = self._df.select(
            infer_polars_schema(
                pl.col(column),
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                merge_schemas=merge_schemas,
                debug=debug,
                verbosity=verbosity,
                map_threshold=map_threshold,
                map_max_required_keys=map_max_required_keys,
                unify_maps=unify_maps,
                **fft,
                wrap_scalars=wrap_scalars,
                avro=avro,
                wrap_root=wrap_root_field,
                no_root_map=no_root_map,
            ).first()
        )

        # Extract the schema from the first column, which is the struct
        schema_fields = result.to_series().item()
        return pl.Schema(
            {
                field["name"]: _parse_polars_dtype(field["dtype"])
                for field in schema_fields
            }
        )

    def infer_json_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        schema_uri: str | None = "http://json-schema.org/schema#",
        merge_schemas: bool = True,
        debug: bool = False,
        verbosity: Literal["Normal", "Verbose"] = "Normal",
        map_threshold: int = 20,
        map_max_required_keys: int | None = None,
        unify_maps: bool = False,
        no_unify: set[str] | None = None,
        force_field_types: dict[str, str] | None = None,
        wrap_scalars: bool = True,
        avro: bool = False,
        wrap_root: bool | str | None = None,
        no_root_map: bool = True,
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
        schema_uri : str or None, default "http://json-schema.org/schema#"
            Schema URI to use for the generated schema
        merge_schemas : bool, default True
            Whether to merge schemas from all rows (True) or return individual schemas (False)
        debug : bool, default False
            Whether to print debug information
        verbosity : str, default "Normal"
            Whether to print verbose debug information
        map_threshold : int, default 20
            Number of keys above which a heterogeneous object may be rewritten
            as a map (unless overridden).
        map_max_required_keys : int, optional
            Maximum number of required keys allowed for Map inference. Objects with more
            required keys will be forced to Record type. If None, no gating based on
            required key count.
        unify_maps : bool, default False
            Enable unification of compatible but non-homogeneous record schemas into maps.
            When True, record schemas with compatible field types can be merged into a single
            map schema with selective nullable fields.
        no_unify: set[str] | None, default None
            Prevent unification of keys under these field names with their sibling record fields.
        force_field_types : dict[str, str], optional
            Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
            Example: ``{"labels": "map", "claims": "record"}``.
        wrap_scalars : bool, default True
            Whether to promote scalar values into singleton objects when they appear
            in contexts where other rows provide objects. This avoids unification
            failures between scalars and objects. The promoted field name defaults
            to the parent key with a ``__{type}`` suffix, e.g. a string under
            ``"value"`` becomes ``{"value__string": "..."}``.
        avro: bool, default False
            Whether to read the input as an Avro schema instead of JSON schema.
        wrap_root : str | bool | None, default None
            If a string, wrap each JSON row under that key before inference.
            If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
        no_root_map : bool, default True
            Prevent document root from becoming a map type, even if it meets map inference criteria

        Returns:
        -------
        dict | list[dict]
            The inferred JSON schema as a dictionary (if merge_schemas=True) or
            list of schemas (if merge_schemas=False)
        """
        wrap_root_field = column if wrap_root is True else wrap_root
        result = self._df.select(
            infer_json_schema(
                pl.col(column),
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                schema_uri=schema_uri,
                merge_schemas=merge_schemas,
                debug=debug,
                verbosity=verbosity,
                map_threshold=map_threshold,
                map_max_required_keys=map_max_required_keys,
                unify_maps=unify_maps,
                force_field_types=force_field_types,
                wrap_scalars=wrap_scalars,
                avro=avro,
                wrap_root=wrap_root_field,
                no_root_map=no_root_map,
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

    def normalise_json(
        self,
        column: str,
        *,
        decode: bool | pl.Schema = True,
        unnest: bool = True,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        empty_as_null: bool = True,
        coerce_strings: bool = False,
        map_encoding: Literal["entries", "mapping", "kv"] = "kv",
        map_threshold: int = 20,
        map_max_required_keys: int | None = None,
        unify_maps: bool = False,
        no_unify: set[str] | None = None,
        force_field_types: dict[str, str] | None = None,
        wrap_scalars: bool = True,
        wrap_root: bool | str | None = None,
        no_root_map: bool = True,
    ) -> pl.Series:
        """Normalise a JSON string column to conform to an inferred Avro schema.

        This is a higher-level wrapper around :func:`normalise_json`, returning the
        results as a Polars Series instead of an expression.

        Parameters
        ----------
        column : str
            Name of the column containing JSON strings.
        decode : bool | pl.Schema, default True
            Controls how the normalised JSON strings are decoded after
            normalisation:

            - If False leave values as raw JSON strings.
            - If True (default), decode into native Polars datatypes,
              with the schema inferred from the data (may be slower).
            - If a polars.Schema, decode using the
              provided schema dtype directly (fast path, skips final schema inference).
        unnest : bool, default True
            Only applies if `decode=True`. If True, expand the decoded struct
            into separate columns for each schema field. If False, keep a
            single Series of structs.
        ignore_outer_array : bool, default True
            Whether to treat a top-level JSON array as a stream of objects instead
            of a single array value.
        ndjson : bool, default False
            Whether the input column contains newline-delimited JSON (NDJSON).
        empty_as_null : bool, default True
            If True, normalise empty arrays and empty maps to ``null``.
            If False, preserve them as empty collections.
        coerce_strings : bool, default False
            If True, attempt to parse numeric/boolean values from strings
            (e.g. ``"42" → 42``, ``"true" → true``). If False, leave them as strings.
        map_encoding : {"mapping", "entries", "kv"}, default "kv"
            Encoding to use for Avro maps:
            - "mapping": plain JSON object ({"en":"Hello"})
            - "entries": list of single-entry objects ([{"en":"Hello"}])
            - "kv":      list of {key,value} dicts ([{"key":"en","value":"Hello"}])
        map_threshold : int, default 20
            Threshold above which objects with many varying keys are normalised
            as Avro maps instead of records.
        map_max_required_keys : int, optional
            Maximum number of required keys allowed for Map inference during schema
            inference. Objects with more required keys will be forced to Record type.
            If None, no gating based on required key count.
        unify_maps : bool, default False
            Enable unification of compatible but non-homogeneous record schemas into maps.
            When True, record schemas with compatible field types can be merged into a single
            map schema with selective nullable fields.
        no_unify: set[str] | None, default None
            Prevent unification of keys under these field names with their sibling record fields.
        force_field_types : dict[str, str], optional
            Per-field overrides for schema inference (e.g. ``{"labels": "map"}``).
        wrap_scalars : bool, default True
            Whether to promote scalar values into singleton objects when they appear
            in contexts where other rows provide objects. This avoids unification
            failures between scalars and objects. The promoted field name defaults
            to the parent key with a ``__{type}`` suffix, e.g. a string under
            ``"value"`` becomes ``{"value__string": "..."}``.
        wrap_root : str | bool | None, default None
            If a string, wrap each JSON row under that key before normalisation.
            If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
        no_root_map : bool, default True
            Prevent document root from becoming a map type, even if it meets map inference criteria

        Returns:
        -------
        pl.Series
            A Series of normalised JSON data. Each row is rewritten to match the
            same Avro schema, with consistent shape across the column.
            If ``unnest=True``, the Series is expanded into multiple columns
            corresponding to schema fields.
        """
        wrap_root_field = column if wrap_root is True else wrap_root
        expr = normalise_json(
            pl.col(column),
            ignore_outer_array=ignore_outer_array,
            ndjson=ndjson,
            empty_as_null=empty_as_null,
            coerce_strings=coerce_strings,
            map_encoding=map_encoding,
            map_threshold=map_threshold,
            map_max_required_keys=map_max_required_keys,
            unify_maps=unify_maps,
            force_field_types=force_field_types,
            wrap_scalars=wrap_scalars,
            wrap_root=wrap_root_field,
            no_root_map=no_root_map,
        )
        if decode:
            if map_encoding != "kv":
                # Map type fields must be k:v encoded as infer_polars_schema assumes it
                # This could be done, it would always make record fields, ...but why?
                raise NotImplementedError("map_encoding must be kv to decode to Polars")

            if decode is True:
                # Infer Avro schema and convert it to Polars Schema
                schema = self.infer_polars_schema(
                    column,
                    ignore_outer_array=ignore_outer_array,
                    ndjson=ndjson,
                    merge_schemas=True,
                    map_threshold=map_threshold,
                    map_max_required_keys=map_max_required_keys,
                    unify_maps=unify_maps,
                    force_field_types=force_field_types,
                    wrap_scalars=wrap_scalars,
                    avro=True,
                    wrap_root=wrap_root_field,
                    no_root_map=no_root_map,
                )
                dtype = pl.Struct(schema)
            else:
                # decode was passed as a Polars Schema directly
                dtype = decode

            result = self._df.select(expr.str.json_decode(dtype=dtype))
            if unnest:
                result = result.unnest(expr.meta.output_name())
        else:
            result = self._df.select(expr).to_series()
        return result
