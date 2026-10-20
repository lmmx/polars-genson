"""Dtype parsing from concise string format used to serialise across Rust to Python."""

import re

import polars as pl


def _parse_polars_dtype(dtype_str: str, map_encoding: str = "mapping") -> pl.DataType:
    """Parse a dtype string like 'Struct[id:Int64,name:String]' into actual Polars DataType.

    A map, ``Map[String,V]``, becomes the dtype its `map_encoding` decodes to: a
    ``pl.Map`` for ``"mapping"``, a list of ``{key, value}`` structs for ``"kv"``.
    """
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
        inner_type = _parse_polars_dtype(inner_type_str, map_encoding)
        return pl.List(inner_type)

    # Handle Map[KeyType,ValueType]
    if dtype_str.startswith("Map[") and dtype_str.endswith("]"):
        key_str, value_str = _split_struct_fields(dtype_str[4:-1])
        key_type = _parse_polars_dtype(key_str, map_encoding)
        value_type = _parse_polars_dtype(value_str, map_encoding)
        if map_encoding == "mapping":
            return pl.Map(key_type, value_type)
        if map_encoding == "kv":
            return pl.List(pl.Struct({"key": key_type, "value": value_type}))
        raise ValueError(f"map_encoding={map_encoding!r} has no Polars dtype")

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
                    inner_type = _parse_polars_dtype(
                        inner_type_str.strip(), map_encoding
                    )
                    return pl.Array(inner_type, size)
                except ValueError:
                    pass
        # Fallback to List if parsing fails
        inner_type = _parse_polars_dtype(inner_str, map_encoding)
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
            field_type = _parse_polars_dtype(field_type_str, map_encoding)
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


def contains_map(dtype: pl.DataType) -> bool:
    """Whether a dtype is, or holds, a `pl.Map`."""
    if isinstance(dtype, pl.Map):
        return True
    if isinstance(dtype, pl.Struct):
        return any(contains_map(field.dtype) for field in dtype.fields)
    if isinstance(dtype, (pl.List, pl.Array)):
        return contains_map(dtype.inner)
    return False


def project_dtype(
    expr: pl.Expr, have: pl.DataType, want: pl.DataType
) -> pl.Expr | None:
    """`expr`, of dtype `have`, as `str.json_decode` decodes the same values to `want`.

    `want`'s struct fields are taken by name (a field `have` lacks is null, one `want`
    lacks is dropped) and lists are projected element by element. ``None`` where the
    result could differ from `json_decode`'s: a leaf, map or array dtype other than
    `have`'s (where a cast and `json_decode`'s parse can disagree), or a nested `want`
    where `have` holds only nulls. Structs are built field by field, never cast, so
    no cast drops a struct field.
    """
    if have == want:
        return expr
    if isinstance(have, pl.Null):
        # Only nulls were seen there, and json_decode makes a null of any leaf dtype
        nested = (pl.Struct, pl.List, pl.Array, pl.Map)
        return None if isinstance(want, nested) else expr.cast(want)
    if isinstance(have, pl.Struct) and isinstance(want, pl.Struct):
        fields = {field.name: field.dtype for field in have.fields}
        parts = []
        for field in want.fields:
            if field.name in fields:
                part = project_dtype(
                    expr.struct.field(field.name), fields[field.name], field.dtype
                )
                if part is None:
                    return None
            else:
                # json_decode gives a field the JSON lacks as null (a null struct, not
                # a struct of nulls)
                part = pl.lit(None, dtype=field.dtype)
            parts.append(part.alias(field.name))
        # A null struct stays null, not a struct of null fields
        return pl.when(expr.is_not_null()).then(pl.struct(parts))
    if isinstance(have, pl.List) and isinstance(want, pl.List):
        inner = project_dtype(pl.element(), have.inner, want.inner)
        return None if inner is None else expr.list.eval(inner)
    return None


def typed_route_makes(dtype: pl.DataType) -> bool:
    """Whether `dtype` holds only dtypes the typed route can decode to.

    Those are the ones ``avro_to_polars_schema`` gives an Avro schema: String, Int64,
    Float64, Boolean and Null leaves, in lists, structs and maps keyed by String. A dtype
    holding any other (Categorical, Int32, Datetime, Decimal, Array, ...) never projects,
    so it is decoded from JSON text without running the typed route first.
    """
    # A field's dtype may be the class itself (pl.String) or an instance
    if any(
        dtype == leaf for leaf in (pl.String, pl.Int64, pl.Float64, pl.Boolean, pl.Null)
    ):
        return True
    if isinstance(dtype, pl.List):
        return typed_route_makes(dtype.inner)
    if isinstance(dtype, pl.Struct):
        return all(typed_route_makes(field.dtype) for field in dtype.fields)
    if isinstance(dtype, pl.Map):
        return dtype.key == pl.String and typed_route_makes(dtype.value)
    return False


def dtype_spec(dtype: pl.DataType | pl.Schema) -> object:
    """Describe a dtype (or a schema, as a struct of its columns) for a target schema.

    The JSON-ready description the Rust side reads (genson_core::conform::
    data_type_from_spec): a dtype name such as ``"String"``, or ``{"Struct": [[name,
    dtype], ...]}``, ``{"List": dtype}``, ``{"Array": [dtype, size]}`` or ``{"Map":
    [key, value]}``.
    """
    if isinstance(dtype, pl.Schema):
        dtype = pl.Struct(dtype)
    if isinstance(dtype, pl.Struct):
        return {"Struct": [[f.name, dtype_spec(f.dtype)] for f in dtype.fields]}
    if isinstance(dtype, pl.List):
        return {"List": dtype_spec(dtype.inner)}
    if isinstance(dtype, pl.Array):
        return {"Array": [dtype_spec(dtype.inner), dtype.size]}
    if isinstance(dtype, pl.Map):
        return {"Map": [dtype_spec(dtype.key), dtype_spec(dtype.value)]}
    return str(dtype.base_type() if not isinstance(dtype, type) else dtype)
