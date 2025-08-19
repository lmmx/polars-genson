import polars as pl


def _parse_polars_dtype(dtype_str: str) -> pl.DataType:
    """Parse a dtype string like 'Struct[id:Int64,name:String]' into actual Polars DataType."""

    # Simple types first
    simple_types = {
        "String": pl.Utf8,
        "Int64": pl.Int64,
        "Int32": pl.Int32,
        "Float64": pl.Float64,
        "Float32": pl.Float32,
        "Boolean": pl.Boolean,
        "Null": pl.Null,
    }

    if dtype_str in simple_types:
        return simple_types[dtype_str]

    # Handle List[ItemType]
    if dtype_str.startswith("List[") and dtype_str.endswith("]"):
        inner_type_str = dtype_str[5:-1]  # Remove "List[" and "]"
        inner_type = _parse_polars_dtype(inner_type_str)
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

    for char in fields_str:
        if char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
        elif char == "," and bracket_depth == 0:
            if current_field.strip():
                fields.append(current_field.strip())
            current_field = ""
            continue

        current_field += char

    if current_field.strip():
        fields.append(current_field.strip())

    return fields
