"""Tests for schema serialization functionality."""

import polars as pl
import polars_genson  # noqa: F401


def test_basic_schema_serialization():
    """Test basic DataFrame schema to JSON Schema conversion."""
    df = pl.DataFrame(
        {
            "name": ["Alice", "Bob", "Charlie"],
            "age": [30, 25, 35],
            "active": [True, False, True],
            "score": [95.5, 87.2, 92.1],
        }
    )

    json_schema = df.genson.serialize_schema_to_json()

    # Verify the structure
    assert isinstance(json_schema, dict)
    assert json_schema["type"] == "object"
    assert "properties" in json_schema
    assert "$schema" in json_schema

    # Check properties
    props = json_schema["properties"]
    assert "name" in props
    assert "age" in props
    assert "active" in props
    assert "score" in props

    # Check types
    assert props["name"]["type"] == "string"
    assert props["age"]["type"] == "integer"
    assert props["active"]["type"] == "boolean"
    assert props["score"]["type"] == "number"

    # Check required fields (all should be required by default)
    required = json_schema.get("required", [])
    assert "name" in required
    assert "age" in required
    assert "active" in required
    assert "score" in required


def test_schema_serialization_with_options():
    """Test schema serialization with custom options."""
    df = pl.DataFrame(
        {
            "id": [1, 2, 3],
            "name": ["Alice", "Bob", "Charlie"],
            "email": ["alice@example.com", "bob@example.com", "charlie@example.com"],
            "phone": [None, "555-1234", None],
        }
    )

    json_schema = df.genson.serialize_schema_to_json(
        title="User Schema",
        description="A schema for user data",
        optional_fields=["email", "phone"],
        additional_properties=True,
        schema_uri=None,  # Omit schema URI
    )

    # Verify custom options
    assert json_schema["title"] == "User Schema"
    assert json_schema["description"] == "A schema for user data"
    assert json_schema["additionalProperties"] is True
    assert "$schema" not in json_schema  # Should be omitted

    # Check required fields (should not include optional ones)
    required = json_schema.get("required", [])
    assert "id" in required
    assert "name" in required
    assert "email" not in required
    assert "phone" not in required


def test_complex_types_serialization():
    """Test serialization of complex Polars types."""
    df = pl.DataFrame(
        {
            "tags": [["python", "rust"], ["javascript"], ["go", "java"]],
            "metadata": [
                {"role": "admin", "active": True},
                {"role": "user", "active": False},
                {"role": "admin", "active": True},
            ],
            "scores": [[1, 2, 3], [4, 5], [6, 7, 8, 9]],
        }
    )

    json_schema = df.genson.serialize_schema_to_json()

    props = json_schema["properties"]

    # Check list types
    assert props["tags"]["type"] == "array"
    assert props["tags"]["items"]["type"] == "string"

    assert props["scores"]["type"] == "array"
    assert props["scores"]["items"]["type"] == "integer"

    # Check struct type
    assert props["metadata"]["type"] == "object"
    assert "properties" in props["metadata"]
    metadata_props = props["metadata"]["properties"]
    assert "role" in metadata_props
    assert "active" in metadata_props
    assert metadata_props["role"]["type"] == "string"
    assert metadata_props["active"]["type"] == "boolean"


def test_datetime_types_serialization():
    """Test serialization of date/time types."""
    df = pl.DataFrame(
        {
            "date_col": [pl.date(2023, 1, 1), pl.date(2023, 1, 2)],
            "datetime_col": [
                pl.datetime(2023, 1, 1, 12, 0, 0),
                pl.datetime(2023, 1, 2, 15, 30, 0),
            ],
        }
    )

    json_schema = df.genson.serialize_schema_to_json()
    props = json_schema["properties"]

    # Check date format
    assert props["date_col"]["type"] == "string"
    assert props["date_col"]["format"] == "date"

    # Check datetime format
    assert props["datetime_col"]["type"] == "string"
    assert props["datetime_col"]["format"] == "date-time"


def test_expression_usage():
    """Test using serialize_polars_schema expression directly."""
    # Create schema data manually
    schema_data = pl.DataFrame(
        {
            "name": ["id", "username", "email"],
            "dtype": ["Int64", "String", "String"],
        }
    )

    result = schema_data.select(
        polars_genson.serialize_polars_schema(
            pl.struct(["name", "dtype"]),
            title="API User Schema",
            optional_fields=["email"],
        ).alias("json_schema")
    )

    json_schema_str = result.get_column("json_schema").first()
    assert isinstance(json_schema_str, str)

    # Parse and verify
    import orjson

    json_schema = orjson.loads(json_schema_str)

    assert json_schema["title"] == "API User Schema"
    assert json_schema["type"] == "object"
    assert "id" in json_schema["properties"]
    assert "username" in json_schema["properties"]
    assert "email" in json_schema["properties"]


def test_empty_dataframe():
    """Test serialization of empty DataFrame."""
    df = pl.DataFrame()

    json_schema = df.genson.serialize_schema_to_json()

    assert json_schema["type"] == "object"
    assert json_schema["properties"] == {}
    assert json_schema.get("required", []) == []


def test_nested_structures():
    """Test serialization of deeply nested structures."""
    df = pl.DataFrame(
        {
            "user": [
                {"profile": {"name": "Alice", "settings": {"theme": "dark"}}},
                {"profile": {"name": "Bob", "settings": {"theme": "light"}}},
            ],
            "posts": [
                [{"title": "Hello", "likes": 5}, {"title": "World", "likes": 3}],
                [{"title": "Test", "likes": 1}],
            ],
        }
    )

    json_schema = df.genson.serialize_schema_to_json()
    props = json_schema["properties"]

    # Check nested struct
    assert props["user"]["type"] == "object"
    user_props = props["user"]["properties"]
    assert "profile" in user_props
    assert user_props["profile"]["type"] == "object"

    # Check array of structs
    assert props["posts"]["type"] == "array"
    assert props["posts"]["items"]["type"] == "object"
    post_props = props["posts"]["items"]["properties"]
    assert "title" in post_props
    assert "likes" in post_props
    assert post_props["title"]["type"] == "string"
    assert post_props["likes"]["type"] == "integer"


def test_debug_output(capsys):
    """Test that debug output works."""
    df = pl.DataFrame(
        {
            "test_col": [1, 2, 3],
        }
    )

    df.genson.serialize_schema_to_json(debug=True)

    # Check that debug output was captured
    captured = capsys.readouterr()
    assert (
        "DEBUG:" in captured.err or len(captured.err) > 0
    )  # Some debug output should appear


def test_schema_consistency():
    """Test that the same DataFrame produces consistent schemas."""
    df1 = pl.DataFrame(
        {
            "name": ["Alice"],
            "age": [30],
        }
    )

    df2 = pl.DataFrame(
        {
            "name": ["Bob"],
            "age": [25],
        }
    )

    schema1 = df1.genson.serialize_schema_to_json()
    schema2 = df2.genson.serialize_schema_to_json()

    # Remove any timestamps or dynamic content for comparison
    def normalize_schema(schema):
        normalized = schema.copy()
        # Remove any keys that might vary between runs
        return normalized

    assert normalize_schema(schema1) == normalize_schema(schema2)
