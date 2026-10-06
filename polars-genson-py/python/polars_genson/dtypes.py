"""Dtype parsing from concise string format used to serialise across Rust to Python."""

import re

import polars as pl

HAS_MAP_DTYPE = hasattr(pl, "Map")
"""Whether this Polars has a native Map dtype (2.0+)."""


def _parse_polars_dtype(dtype_str: str) -> pl.DataType:
    """Parse a dtype string like 'Struct[id:Int64,name:String]' into actual Polars DataType."""
    dtype_str = dtype_str.strip()

    # Handle Decimal(precision, scale)
    if dtype_str.startswith("Decimal"):
        # Match Decimal(p, s) pattern
        m = re.match(r"Decimal\s*\(\s*(\d+)\s*,\s*(\d+)\s*\)", dtype_str)
        if m:
            precision, scale = int(m.group(1)), int(m.group(2))
            return pl.Decimal(precision, scale)
        elif dtype_str == "Decimal":
            # Just "Decimal" without parameters
            return pl.Decimal(None, None)
        else:
            # Fallback for malformed Decimal
            return pl.Decimal(None, None)

    # Simple types first
    simple_types = {
        "String": pl.Utf8,
        "Int64": pl.Int64,
        "Int32": pl.Int32,
        "Int16": pl.Int16,
        "Int8": pl.Int8,
        "UInt64": pl.UInt64,
        "UInt32": pl.UInt32,
        "UInt16": pl.UInt16,
        "UInt8": pl.UInt8,
        "Float64": pl.Float64,
        "Float32": pl.Float32,
        "Boolean": pl.Boolean,
        "Date": pl.Date,
        "Time": pl.Time,
        "Datetime": pl.Datetime,
        "Duration": pl.Duration,
        "Null": pl.Null,
        "Binary": pl.Binary,
        "Categorical": pl.Categorical,
    }

    if dtype_str in simple_types:
        return simple_types[dtype_str]

    # Handle List[ItemType]
    if dtype_str.startswith("List[") and dtype_str.endswith("]"):
        inner_type_str = dtype_str[5:-1]  # Remove "List[" and "]"
        inner_type = _parse_polars_dtype(inner_type_str)
        return pl.List(inner_type)

    # Handle Map[String,ValueType]: a real `pl.Map` on Polars >= 2, otherwise the
    # list-of-{key,value}-structs encoding that older Polars uses to represent maps
    if dtype_str.startswith("Map[") and dtype_str.endswith("]"):
        key_str, value_str = _split_struct_fields(dtype_str[4:-1])
        value_type = _parse_polars_dtype(value_str)
        if HAS_MAP_DTYPE:
            return pl.Map(_parse_polars_dtype(key_str), value_type)
        return pl.List(
            pl.Struct([pl.Field("key", pl.Utf8), pl.Field("value", value_type)])
        )

    # Handle Array[ItemType,Size]
    if dtype_str.startswith("Array[") and dtype_str.endswith("]"):
        inner_str = dtype_str[6:-1]  # Remove "Array[" and "]"
        # Find the last comma to split type and size
        if "," in inner_str:
            parts = inner_str.rsplit(",", 1)
            if len(parts) == 2:
                inner_type_str, size_str = parts
                try:
                    size = int(size_str.strip())
                    inner_type = _parse_polars_dtype(inner_type_str.strip())
                    return pl.Array(inner_type, size)
                except ValueError:
                    pass
        # Fallback to List if parsing fails
        inner_type = _parse_polars_dtype(inner_str)
        return pl.List(inner_type)

    # Handle Struct[field1:Type1,field2:Type2,...]
    if dtype_str.startswith("Struct[") and dtype_str.endswith("]"):
        fields_str = dtype_str[7:-1]  # Remove "Struct[" and "]"

        if not fields_str:  # Empty struct
            return pl.Struct([])

        # Parse field definitions
        fields = []
        # Split by comma but be careful of nested types
        field_parts = _split_struct_fields(fields_str)

        for field_part in field_parts:
            if ":" not in field_part:
                continue
            field_name, field_type_str = field_part.split(":", 1)
            field_name = field_name.strip()
            field_type_str = field_type_str.strip()
            field_type = _parse_polars_dtype(field_type_str)
            fields.append(pl.Field(field_name, field_type))

        return pl.Struct(fields)

    # Fallback to String for unknown types
    return pl.Utf8


def _split_struct_fields(fields_str: str) -> list[str]:
    """Split struct field definitions by comma, handling nested brackets."""
    fields = []
    current_field = ""
    bracket_depth = 0
    paren_depth = 0

    for char in fields_str:
        if char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
        elif char == "(":
            paren_depth += 1
        elif char == ")":
            paren_depth -= 1
        elif char == "," and bracket_depth == 0 and paren_depth == 0:
            if current_field.strip():
                fields.append(current_field.strip())
            current_field = ""
            continue

        current_field += char

    if current_field.strip():
        fields.append(current_field.strip())

    return fields


def contains_map_dtype(dtype: pl.DataType) -> bool:
    """Whether a dtype is, or nests, a `pl.Map`."""
    if not HAS_MAP_DTYPE:
        return False
    if isinstance(dtype, pl.Map):
        return True
    if isinstance(dtype, pl.Struct):
        return any(contains_map_dtype(f.dtype) for f in dtype.fields)
    if isinstance(dtype, (pl.List, pl.Array)):
        return contains_map_dtype(dtype.inner)
    return False


def default_map_encoding() -> str:
    """The map encoding that matches the dtype of inferred maps on this Polars.

    Maps are `pl.Map` on Polars 2+ (decoded from plain JSON objects, i.e. ``mapping``)
    and lists of ``{key, value}`` structs before that (``kv``).
    """
    return "mapping" if HAS_MAP_DTYPE else "kv"


def maps_to_entries(dtype: pl.DataType) -> pl.DataType:
    """Rewrite each nested `pl.Map` as the list of ``{key, value}`` structs."""
    if not contains_map_dtype(dtype):
        return dtype
    if isinstance(dtype, pl.Map):
        return pl.List(
            pl.Struct(
                [
                    pl.Field("key", maps_to_entries(dtype.key)),
                    pl.Field("value", maps_to_entries(dtype.value)),
                ]
            )
        )
    if isinstance(dtype, pl.Struct):
        return pl.Struct(
            [pl.Field(f.name, maps_to_entries(f.dtype)) for f in dtype.fields]
        )
    if isinstance(dtype, pl.Array):
        return pl.Array(maps_to_entries(dtype.inner), dtype.size)
    return pl.List(maps_to_entries(dtype.inner))
