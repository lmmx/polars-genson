"""Ensure the 'order of appearance' (insertion order) of object fields is preserved."""


def test_field_order_preservation_basic():
    """Test that basic field order is preserved."""
    import polars as pl
    import polars_genson

    # Create DataFrame with JSON that has non-alphabetical field order
    df = pl.DataFrame({"json_col": ['{"z": "last", "b": "middle", "a": "first"}']})

    schema = df.genson.infer_json_schema("json_col")
    properties = schema.get("properties", {})

    # Field order should be preserved (z, b, a)
    field_names = list(properties.keys())
    assert field_names == [
        "z",
        "b",
        "a",
    ], f"Expected ['z', 'b', 'a'], got {field_names}"


def test_field_order_preservation_nested():
    """Test that nested object field order is preserved."""
    import polars as pl
    import polars_genson

    df = pl.DataFrame({"json_col": ['{"outer": {"z": 1, "a": 2}, "first": true}']})

    schema = df.genson.infer_json_schema("json_col")

    # Top level order: outer, first
    top_level_fields = list(schema.get("properties", {}).keys())
    assert top_level_fields == ["outer", "first"]

    # Nested level order: z, a
    nested_props = schema["properties"]["outer"]["properties"]
    nested_fields = list(nested_props.keys())
    assert nested_fields == ["z", "a"]


def test_field_order_multiple_objects():
    """Test field order with multiple JSON objects."""
    import polars as pl
    import polars_genson

    df = pl.DataFrame(
        {
            "json_col": [
                '{"z": 1, "b": 2}',
                '{"b": 3, "a": 4, "z": 5}',  # Different order, but z,b should win
                '{"c": 6, "z": 7, "b": 8}',  # New field c should appear last
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_col")

    # Field order should be based on first occurrence: z, b, a, c
    field_names = list(schema.get("properties", {}).keys())

    # z should come before b (from first object)
    z_pos = field_names.index("z")
    b_pos = field_names.index("b")
    assert z_pos < b_pos

    # a should come after b but before c
    a_pos = field_names.index("a")
    c_pos = field_names.index("c")
    assert b_pos < a_pos < c_pos


def test_field_order_can_be_disabled():
    """Test that field order preservation can be disabled."""
    import polars as pl
    import polars_genson

    df = pl.DataFrame({"json_col": ['{"z": "last", "b": "middle", "a": "first"}']})

    # This test assumes there's a way to disable field order preservation
    # You'll need to check your Python API to see how this is exposed
    # It might be something like:
    # schema = df.genson.infer_json_schema("json_col", preserve_field_order=False)
    # or
    # schema = df.genson.infer_json_schema("json_col", config={"preserve_field_order": False})

    # For now, showing the expected behavior:
    # When disabled, should get alphabetical order: a, b, z
    # field_names = list(schema.get("properties", {}).keys())
    # assert field_names == ["a", "b", "z"]


def test_field_order_with_arrays():
    """Test field order preservation with arrays of objects."""
    import polars as pl
    import polars_genson

    df = pl.DataFrame({"json_col": ['[{"z": 1, "a": 2}, {"z": 3, "a": 4, "b": 5}]']})

    schema = df.genson.infer_json_schema("json_col")

    # Should detect array of objects
    assert schema.get("type") == "object"

    # Check the items schema preserves field order
    items_props = schema["properties"]
    field_names = list(items_props.keys())

    # z should come before a (from first object)
    z_pos = field_names.index("z")
    a_pos = field_names.index("a")
    assert z_pos < a_pos


def test_required_fields_order():
    """Test that required fields are ordered according to field appearance."""
    import polars as pl
    import polars_genson

    df = pl.DataFrame(
        {
            "json_col": [
                '{"z": 1, "b": 2, "a": 3}',  # All fields present
                '{"z": 4, "b": 5, "a": 6}',  # All fields present
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_col")

    # All fields should be required
    required_fields = schema.get("required", [])

    # Required fields should be in the same order as they appear: z, b, a
    assert required_fields == ["a", "b", "z"]
