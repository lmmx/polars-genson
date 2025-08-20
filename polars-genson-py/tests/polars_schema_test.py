"""Test the production of Polars schemas via schema inference."""

import polars as pl
import pytest
from polars_genson import infer_polars_schema


def test_basic_schema_inference():
    """Test basic JSON schema inference with simple types."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "name": "Alice", "age": 30}',
                '{"id": 2, "name": "Bob", "age": 25}',
                '{"id": 3, "name": "Charlie", "age": 35}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "id": pl.Int64,
            "name": pl.String,
            "age": pl.Int64,
        }
    )


def test_mixed_types():
    """Test with mixed JSON types including floats and booleans."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "name": "Alice", "score": 95.5, "active": true}',
                '{"id": 2, "name": "Bob", "score": 87.2, "active": false}',
                '{"id": 3, "name": "Charlie", "score": 92.1, "active": true}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "id": pl.Int64,
            "name": pl.String,
            "score": pl.Float64,
            "active": pl.Boolean,
        }
    )


def test_nested_objects():
    """Test with nested objects/structs."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"user": {"id": 1, "name": "Alice"}, "metadata": {"created": "2023-01-01"}}',
                '{"user": {"id": 2, "name": "Bob"}, "metadata": {"created": "2023-01-02"}}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "user": pl.Struct({"id": pl.Int64, "name": pl.String}),
            "metadata": pl.Struct({"created": pl.String}),
        }
    )


def test_arrays():
    """Test with arrays of different types."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "tags": ["python", "rust"], "scores": [1, 2, 3]}',
                '{"id": 2, "tags": ["javascript"], "scores": [4, 5]}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "id": pl.Int64,
            "tags": pl.List(pl.String),
            "scores": pl.List(pl.Int64),
        }
    )


def test_complex_nested_structure():
    """Test with deeply nested structures."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"user": {"profile": {"name": "Alice", "settings": {"theme": "dark"}}}, "posts": [{"title": "Hello", "likes": 5}]}',
                '{"user": {"profile": {"name": "Bob", "settings": {"theme": "light"}}}, "posts": [{"title": "World", "likes": 3}]}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "user": pl.Struct(
                {
                    "profile": pl.Struct(
                        {"name": pl.String, "settings": pl.Struct({"theme": pl.String})}
                    )
                }
            ),
            "posts": pl.List(pl.Struct({"title": pl.String, "likes": pl.Int64})),
        }
    )


def test_optional_fields():
    """Test with optional fields (some objects missing certain keys)."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "name": "Alice", "email": "alice@example.com"}',
                '{"id": 2, "name": "Bob"}',  # Missing email
                '{"id": 3, "name": "Charlie", "email": "charlie@example.com", "age": 30}',  # Has age
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "id": pl.Int64,
            "name": pl.String,
            "email": pl.String,
            "age": pl.Int64,
        }
    )


def test_mixed_array_types():
    """Test with arrays containing different types."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"mixed_numbers": [1, 2.5, 3], "string_array": ["a", "b", "c"]}',
                '{"mixed_numbers": [4.1, 5, 6.7], "string_array": ["d", "e"]}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "mixed_numbers": pl.List(pl.Float64),
            "string_array": pl.List(pl.String),
        }
    )


def test_empty_objects_and_arrays():
    """Test with empty objects and arrays."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"empty_obj": {}, "empty_array": [], "data": {"value": 42}}',
                '{"empty_obj": {}, "empty_array": [], "data": {"value": 84}}',
            ]
        }
    )

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "empty_obj": pl.String,
            "empty_array": pl.List(pl.String),
            "data": pl.Struct({"value": pl.Int64}),
        }
    )


def test_schema_consistency():
    """Test that the same schema is returned for identical structure."""
    df1 = pl.DataFrame({"json_col": ['{"a": 1, "b": "test"}']})

    df2 = pl.DataFrame({"json_col": ['{"a": 2, "b": "different"}']})

    schema1 = df1.genson.infer_polars_schema("json_col")
    schema2 = df2.genson.infer_polars_schema("json_col")

    assert schema1 == schema2
    assert schema1 == pl.Schema(
        {
            "a": pl.Int64,
            "b": pl.String,
        }
    )


def test_single_row():
    """Test schema inference with just one row."""
    df = pl.DataFrame({"json_col": ['{"single": {"nested": {"value": [1, 2, 3]}}}']})

    schema = df.genson.infer_polars_schema("json_col")

    assert schema == pl.Schema(
        {
            "single": pl.Struct({"nested": pl.Struct({"value": pl.List(pl.Int64)})}),
        }
    )
