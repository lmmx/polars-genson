"""A Polars plugin for JSON schema inference from string columns using genson-rs."""

from __future__ import annotations

import inspect
from pathlib import Path

import orjson
import polars as pl
from polars.api import register_dataframe_namespace
from polars.plugins import register_plugin_function

from .dtypes import _parse_polars_dtype
from .utils import parse_into_expr, parse_version  # noqa: F401

# Determine the correct plugin path
if parse_version(pl.__version__) < parse_version("0.20.16"):
    from polars.utils.udfs import _get_shared_lib_location

    lib: str | Path = _get_shared_lib_location(__file__)
else:
    lib = Path(__file__).parent

__all__ = ["infer_json_schema", "infer_polars_schema", "serialize_polars_schema"]


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


def serialize_polars_schema(
    expr: pl.Expr,
    *,
    schema_uri: str | None = "https://json-schema.org/draft/2020-12/schema",
    title: str | None = None,
    description: str | None = None,
    optional_fields: list[str] | None = None,
    additional_properties: bool = False,
    debug: bool = False,
) -> pl.Expr:
    """Serialize Polars schema fields to JSON Schema.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a struct column with 'name' and 'dtype' fields
    schema_uri : str or None, default "https://json-schema.org/draft/2020-12/schema"
        Schema URI to use for the generated schema (None to omit)
    title : str or None, default None
        Title for the JSON Schema
    description : str or None, default None
        Description for the JSON Schema
    optional_fields : list[str] or None, default None
        List of field names that should be optional (not required)
    additional_properties : bool, default False
        Whether to allow additional properties in the schema
    debug : bool, default False
        Whether to print debug information

    Returns:
    -------
    pl.Expr
        Expression representing the JSON Schema as a string
    """
    kwargs = {
        "additional_properties": additional_properties,
        "debug": debug,
    }

    if schema_uri is not None:
        kwargs["schema_uri"] = schema_uri
    if title is not None:
        kwargs["title"] = title
    if description is not None:
        kwargs["description"] = description
    if optional_fields is not None:
        kwargs["optional_fields"] = optional_fields

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

    def serialize_schema_to_json(
        self,
        *,
        schema_uri: str | None = "https://json-schema.org/draft/2020-12/schema",
        title: str | None = None,
        description: str | None = None,
        optional_fields: list[str] | None = None,
        additional_properties: bool = False,
        debug: bool = False,
    ) -> dict:
        """Serialize the DataFrame's schema to JSON Schema format.

        Parameters
        ----------
        schema_uri : str or None, default "https://json-schema.org/draft/2020-12/schema"
            Schema URI to use for the generated schema (None to omit)
        title : str or None, default None
            Title for the JSON Schema
        description : str or None, default None
            Description for the JSON Schema
        optional_fields : list[str] or None, default None
            List of field names that should be optional (not required)
        additional_properties : bool, default False
            Whether to allow additional properties in the schema
        debug : bool, default False
            Whether to print debug information

        Returns:
        -------
        dict
            The JSON Schema as a dictionary
        """
        # Handle empty DataFrame case
        if self._df.is_empty() or len(self._df.schema) == 0:
            base_schema = {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": additional_properties,
            }

            if schema_uri is not None:
                base_schema["$schema"] = schema_uri
            if title is not None:
                base_schema["title"] = title
            if description is not None:
                base_schema["description"] = description

            return base_schema

        # Convert the DataFrame schema to the struct format expected by serialize_polars_schema
        schema_data = []
        for name, dtype in self._df.schema.items():
            # Convert DataType to string representation
            dtype_str = _polars_dtype_to_string(dtype)
            schema_data.append({"name": name, "dtype": dtype_str})

        # Create a temporary DataFrame with the schema information
        schema_df = pl.DataFrame(schema_data)

        # Create a struct column from the schema data
        result = schema_df.select(
            serialize_polars_schema(
                pl.struct(["name", "dtype"]),
                schema_uri=schema_uri,
                title=title,
                description=description,
                optional_fields=optional_fields or [],
                additional_properties=additional_properties,
                debug=debug,
            ).first()
        )

        # Extract and parse the JSON schema
        json_schema_str = result.to_series().item()
        if not isinstance(json_schema_str, str):
            raise ValueError(
                f"Expected string JSON schema, got {type(json_schema_str)}"
            )

        try:
            return orjson.loads(json_schema_str)
        except orjson.JSONDecodeError as e:
            raise ValueError(f"Failed to parse JSON schema: {e}") from e


def _polars_dtype_to_string(dtype: pl.DataType) -> str:
    """Convert a Polars DataType to its string representation."""
    if isinstance(dtype, type):
        # Handle basic types
        if dtype == pl.String:
            return "String"
        elif dtype == pl.Int64:
            return "Int64"
        elif dtype == pl.Int32:
            return "Int32"
        elif dtype == pl.Float64:
            return "Float64"
        elif dtype == pl.Float32:
            return "Float32"
        elif dtype == pl.Boolean:
            return "Boolean"
        elif dtype == pl.Date:
            return "Date"
        elif dtype == pl.Time:
            return "Time"
        elif dtype == pl.Null:
            return "Null"
        else:
            return "String"  # Fallback
    else:
        # Handle complex types (instances)
        if hasattr(dtype, "__class__"):
            class_name = dtype.__class__.__name__
            if class_name == "List":
                inner_type = _polars_dtype_to_string(dtype.inner)
                return f"List[{inner_type}]"
            elif class_name == "Array":
                inner_type = _polars_dtype_to_string(dtype.inner)
                return f"Array[{inner_type},{dtype.width}]"
            elif class_name == "Struct":
                field_strs = []
                for field in dtype.fields:
                    field_type = _polars_dtype_to_string(field.dtype)
                    field_strs.append(f"{field.name}:{field_type}")
                return f"Struct[{','.join(field_strs)}]"
            elif class_name == "Datetime":
                return "Datetime"
            elif class_name == "Duration":
                return "Duration"
            elif class_name == "Categorical":
                return "Categorical"
            elif class_name == "Enum":
                return "Enum"
            elif class_name == "Decimal":
                return "Decimal"

        # Fallback to string representation
        return str(dtype)
