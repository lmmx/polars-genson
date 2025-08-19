"""Core tests for polars-genson plugin."""

import orjson
import polars as pl

# Import the plugin to register the namespace
import polars_genson  # noqa: F401
from pytest import raises


def test_namespace_registration():
    """Test that the genson namespace is registered on DataFrames."""
    df = pl.DataFrame({"test": ["data"]})
    assert hasattr(df, "genson")
    assert hasattr(df.genson, "infer_json_schema")


def test_basic_schema_inference():
    """Test basic JSON schema inference functionality."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}',
                '{"name": "Bob", "age": 25, "city": "NYC"}',
                '{"name": "Charlie", "age": 35, "email": "charlie@example.com"}',
            ]
        }
    )

    # Infer schema using the namespace
    schema = df.genson.infer_json_schema("json_data")

    # Verify schema structure
    assert isinstance(schema, dict)
    assert "type" in schema
    assert "properties" in schema

    # Check that all expected properties are present
    props = schema["properties"]
    assert "name" in props
    assert "age" in props
    assert "city" in props  # Should be present due to merging
    assert "email" in props  # Should be present due to merging

    # Check types
    assert props["name"]["type"] == "string"
    assert props["age"]["type"] == "integer"


def test_empty_column():
    """Test handling of empty JSON column."""
    df = pl.DataFrame({"json_data": []})

    with raises(Exception):  # Should raise an error for empty input
        df.genson.infer_json_schema("json_data")


def test_null_values():
    """Test handling of null values in JSON column."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}',
                None,
                '{"name": "Bob", "age": 25}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data")

    # Should work despite null values
    assert isinstance(schema, dict)
    assert "properties" in schema
    props = schema["properties"]
    assert "name" in props
    assert "age" in props


def test_expression_usage():
    """Test using the expression directly."""
    df = pl.DataFrame(
        {
            "json_col": [
                '{"user_id": 123, "active": true}',
                '{"user_id": 456, "active": false, "premium": true}',
            ]
        }
    )

    # Use the expression directly
    result = df.select(
        polars_genson.infer_json_schema(pl.col("json_col")).alias("schema")
    )

    # Extract and parse the schema
    schema_json = result.get_column("schema").first()
    schema = orjson.loads(schema_json)

    assert "properties" in schema
    props = schema["properties"]
    assert "user_id" in props
    assert "active" in props
    assert props["user_id"]["type"] == "integer"
    assert props["active"]["type"] == "boolean"


def test_invalid_json():
    """Test handling of invalid JSON."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice"}',
                '{"invalid": json}',  # Invalid JSON
                '{"name": "Bob"}',
            ]
        }
    )

    # Should handle invalid JSON gracefully or raise appropriate error
    with raises(Exception):
        df.genson.infer_json_schema("json_data")


def test_invalid_ndjson_format():
    """Test ndjson=True with invalid JSON on one line."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}\n{"invalid": json}\n{"name": "Charlie", "age": 35}',
                '{"name": "Bob", "age": 25}',
            ]
        }
    )

    # Should raise an error due to invalid JSON on the second line of first string
    with raises(Exception) as exc_info:
        df.genson.infer_json_schema("json_data", ndjson=True)

    # Verify the error message contains information about the invalid JSON
    error_message = str(exc_info.value)
    assert "Invalid JSON input" in error_message or "JSON" in error_message


def test_non_string_column():
    """Test error handling for non-string columns."""
    df = pl.DataFrame({"numbers": [1, 2, 3]})

    with raises(Exception):
        df.genson.infer_json_schema("numbers")
