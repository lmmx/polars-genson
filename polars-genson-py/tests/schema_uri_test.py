"""Tests for schema_uri parameter handling."""

import polars as pl
import polars_genson  # noqa: F401
from pytest import mark


def test_schema_uri_default_auto():
    """Test that schema_uri default produces generic schema URI."""
    df = pl.DataFrame({"json_data": ['{"name": "Alice", "age": 30}']})

    # Default behavior (schema_uri=None)
    schema = df.genson.infer_json_schema("json_data")

    # Should have a proper $schema URI starting with http
    assert "$schema" in schema
    assert schema["$schema"] == "http://json-schema.org/schema#"


def test_schema_uri_explicit_none():
    """Test explicit schema_uri=None omits schema URI."""
    df = pl.DataFrame({"json_data": ['{"name": "Alice", "age": 30}']})

    schema = df.genson.infer_json_schema("json_data", schema_uri=None)

    # Should have a proper $schema URI starting with http
    assert "$schema" not in schema


def test_schema_uri_custom_string():
    """Test custom schema_uri string."""
    df = pl.DataFrame({"json_data": ['{"name": "Alice", "age": 30}']})

    custom_uri = "https://example.com/my-schema"
    schema = df.genson.infer_json_schema("json_data", schema_uri=custom_uri)

    assert schema["$schema"] == custom_uri
