"""Tests for JSON array handling."""

import polars as pl
import polars_genson  # noqa: F401


def test_ignore_outer_array_true():
    """Test ignore_outer_array=True (default behavior)."""
    df = pl.DataFrame(
        {
            "json_data": [
                '[{"id": 1, "value": "a"}, {"id": 2, "value": "b"}]',
                '{"single": "object"}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data", ignore_outer_array=True)

    # Should treat array elements as individual objects
    assert isinstance(schema, dict)
    assert "properties" in schema

    props = schema["properties"]
    # Should have properties from both array elements and the single object
    assert "id" in props or "single" in props


def test_ignore_outer_array_false():
    """Test ignore_outer_array=False."""
    df = pl.DataFrame(
        {
            "json_data": [
                '[{"id": 1, "value": "a"}, {"id": 2, "value": "b"}]',
                '{"single": "object"}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data", ignore_outer_array=False)

    # Should treat arrays as arrays, not flatten them
    assert isinstance(schema, dict)
    assert "properties" in schema


def test_ndjson_format():
    """Test ndjson=True for newline-delimited JSON."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}\n{"name": "Bob", "age": 25}',
                '{"name": "Charlie", "age": 35}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data", debug=True, ndjson=True)

    assert isinstance(schema, dict)
    assert "properties" in schema

    props = schema["properties"]
    assert "name" in props
    assert "age" in props


def test_mixed_json_formats():
    """Test handling of mixed JSON formats in same column."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"single": "object"}',
                '[{"array": "element1"}, {"array": "element2"}]',
                '{"another": "single", "value": 42}',
            ]
        }
    )

    # With ignore_outer_array=True, should handle mixed formats
    schema = df.genson.infer_json_schema("json_data", ignore_outer_array=True)

    assert isinstance(schema, dict)
    assert "properties" in schema


def test_empty_arrays():
    """Test handling of empty JSON arrays."""
    df = pl.DataFrame({"json_data": ["[]", '{"valid": "object"}']})

    # Should handle empty arrays gracefully
    schema = df.genson.infer_json_schema("json_data")

    assert isinstance(schema, dict)


def test_nested_arrays():
    """Test handling of nested arrays."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"items": [{"id": 1, "tags": ["red", "blue"]}, {"id": 2, "tags": ["green"]}]}',
                '{"items": [{"id": 3, "tags": ["yellow", "purple", "orange"]}]}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data")

    assert isinstance(schema, dict)
    assert "properties" in schema

    # Should have inferred the nested structure
    props = schema["properties"]
    assert "items" in props
