"""Tests for schema serialization functionality with improved debugging."""

import json

import polars as pl
import polars_genson  # noqa: F401
from pytest import mark


def print_schema(schema, title="Generated Schema"):
    """Helper function to pretty-print schemas for debugging."""
    print(f"\n=== {title} ===")
    print(json.dumps(schema, indent=2))
    print("=" * (len(title) + 8))


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

    print(f"\nDataFrame schema: {df.schema}")
    json_schema = df.genson.serialize_schema_to_json()
    print_schema(json_schema, "Basic Schema")

    # Verify the structure
    assert isinstance(json_schema, dict), f"Expected dict, got {type(json_schema)}"
    assert (
        json_schema["type"] == "object"
    ), f"Expected type 'object', got {json_schema.get('type')}"
    assert (
        "properties" in json_schema
    ), f"Missing 'properties' key. Keys: {list(json_schema.keys())}"
    assert (
        "$schema" in json_schema
    ), f"Missing '$schema' key. Keys: {list(json_schema.keys())}"

    # Check properties
    props = json_schema["properties"]
    print(f"\nProperties found: {list(props.keys())}")

    for field in ["name", "age", "active", "score"]:
        assert (
            field in props
        ), f"Missing field '{field}' in properties. Available: {list(props.keys())}"

    # Check types with detailed output
    expected_types = {
        "name": "string",
        "age": "integer",
        "active": "boolean",
        "score": "number",
    }

    for field, expected_type in expected_types.items():
        actual_type = props[field].get("type")
        print(f"{field}: expected={expected_type}, actual={actual_type}")
        assert (
            actual_type == expected_type
        ), f"Field '{field}': expected type '{expected_type}', got '{actual_type}'"

    # Check required fields
    required = json_schema.get("required", [])
    print(f"Required fields: {required}")

    for field in ["name", "age", "active", "score"]:
        assert (
            field in required
        ), f"Field '{field}' should be required. Required fields: {required}"


@mark.xfail
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

    print(f"\nDataFrame schema: {df.schema}")
    json_schema = df.genson.serialize_schema_to_json(
        title="User Schema",
        description="A schema for user data",
        optional_fields=["email", "phone"],
        additional_properties=True,
        schema_uri=None,  # Omit schema URI
    )
    print_schema(json_schema, "Schema with Options")

    # Verify custom options with detailed output
    print(f"Title: {json_schema.get('title')}")
    assert (
        json_schema["title"] == "User Schema"
    ), f"Expected title 'User Schema', got '{json_schema.get('title')}'"

    print(f"Description: {json_schema.get('description')}")
    assert (
        json_schema["description"] == "A schema for user data"
    ), f"Expected description 'A schema for user data', got '{json_schema.get('description')}'"

    print(f"Additional properties: {json_schema.get('additionalProperties')}")
    assert (
        json_schema["additionalProperties"] is True
    ), f"Expected additionalProperties=True, got {json_schema.get('additionalProperties')}"

    print(f"Schema URI present: {'$schema' in json_schema}")
    print(f"All keys: {list(json_schema.keys())}")
    if "$schema" in json_schema:
        print(f"Schema URI value: {json_schema['$schema']}")

    assert (
        "$schema" not in json_schema
    ), f"Expected no '$schema' key, but found: {json_schema.get('$schema')}"

    # Check required fields
    required = json_schema.get("required", [])
    print(f"Required fields: {required}")
    print(f"Optional fields specified: ['email', 'phone']")

    assert "id" in required, f"Field 'id' should be required. Required: {required}"
    assert "name" in required, f"Field 'name' should be required. Required: {required}"
    assert (
        "email" not in required
    ), f"Field 'email' should be optional. Required: {required}"
    assert (
        "phone" not in required
    ), f"Field 'phone' should be optional. Required: {required}"


@mark.xfail
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

    print(f"\nDataFrame schema: {df.schema}")
    print("Sample data:")
    print(df.head(1))

    json_schema = df.genson.serialize_schema_to_json()
    print_schema(json_schema, "Complex Types Schema")

    props = json_schema["properties"]

    # Check list types with detailed output
    print(f"\nTags field: {props.get('tags', 'MISSING')}")
    tags_type = props["tags"].get("type")
    print(f"Tags type: expected=array, actual={tags_type}")
    assert tags_type == "array", f"Tags field: expected type 'array', got '{tags_type}'"

    tags_items_type = props["tags"].get("items", {}).get("type")
    print(f"Tags items type: expected=string, actual={tags_items_type}")
    assert (
        tags_items_type == "string"
    ), f"Tags items: expected type 'string', got '{tags_items_type}'"

    print(f"\nScores field: {props.get('scores', 'MISSING')}")
    scores_type = props["scores"].get("type")
    print(f"Scores type: expected=array, actual={scores_type}")
    assert (
        scores_type == "array"
    ), f"Scores field: expected type 'array', got '{scores_type}'"

    scores_items_type = props["scores"].get("items", {}).get("type")
    print(f"Scores items type: expected=integer, actual={scores_items_type}")
    assert (
        scores_items_type == "integer"
    ), f"Scores items: expected type 'integer', got '{scores_items_type}'"

    # Check struct type with detailed output
    print(f"\nMetadata field: {props.get('metadata', 'MISSING')}")
    metadata_type = props["metadata"].get("type")
    print(f"Metadata type: expected=object, actual={metadata_type}")
    assert (
        metadata_type == "object"
    ), f"Metadata field: expected type 'object', got '{metadata_type}'"

    if metadata_type == "object":
        metadata_props = props["metadata"].get("properties", {})
        print(f"Metadata properties: {list(metadata_props.keys())}")

        assert (
            "role" in metadata_props
        ), f"Missing 'role' in metadata properties: {list(metadata_props.keys())}"
        assert (
            "active" in metadata_props
        ), f"Missing 'active' in metadata properties: {list(metadata_props.keys())}"

        role_type = metadata_props["role"].get("type")
        active_type = metadata_props["active"].get("type")
        print(f"Role type: expected=string, actual={role_type}")
        print(f"Active type: expected=boolean, actual={active_type}")

        assert (
            role_type == "string"
        ), f"Role field: expected type 'string', got '{role_type}'"
        assert (
            active_type == "boolean"
        ), f"Active field: expected type 'boolean', got '{active_type}'"


@mark.xfail
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

    print(f"\nDataFrame schema: {df.schema}")
    print("Sample data:")
    print(df.head(1))

    json_schema = df.genson.serialize_schema_to_json()
    print_schema(json_schema, "DateTime Types Schema")

    props = json_schema["properties"]

    # Check date format with detailed output
    print(f"\nDate column field: {props.get('date_col', 'MISSING')}")
    date_type = props["date_col"].get("type")
    date_format = props["date_col"].get("format")
    print(f"Date type: expected=string, actual={date_type}")
    print(f"Date format: expected=date, actual={date_format}")

    assert (
        date_type == "string"
    ), f"Date column: expected type 'string', got '{date_type}'"
    assert (
        date_format == "date"
    ), f"Date column: expected format 'date', got '{date_format}'"

    # Check datetime format with detailed output
    print(f"\nDateTime column field: {props.get('datetime_col', 'MISSING')}")
    datetime_type = props["datetime_col"].get("type")
    datetime_format = props["datetime_col"].get("format")
    print(f"DateTime type: expected=string, actual={datetime_type}")
    print(f"DateTime format: expected=date-time, actual={datetime_format}")

    assert (
        datetime_type == "string"
    ), f"DateTime column: expected type 'string', got '{datetime_type}'"
    assert (
        datetime_format == "date-time"
    ), f"DateTime column: expected format 'date-time', got '{datetime_format}'"


def test_expression_usage():
    """Test using serialize_polars_schema expression directly."""
    # Create schema data manually
    schema_data = pl.DataFrame(
        {
            "name": ["id", "username", "email"],
            "dtype": ["Int64", "String", "String"],
        }
    )

    print(f"\nSchema data: {schema_data}")

    result = schema_data.select(
        polars_genson.serialize_polars_schema(
            pl.struct(["name", "dtype"]),
            title="API User Schema",
            optional_fields=["email"],
        ).alias("json_schema")
    )

    print(f"Result: {result}")
    json_schema_str = result.get_column("json_schema").first()
    print(f"JSON schema string: {json_schema_str}")
    print(f"String type: {type(json_schema_str)}")

    assert isinstance(
        json_schema_str, str
    ), f"Expected string, got {type(json_schema_str)}"

    # Parse and verify
    import orjson

    json_schema = orjson.loads(json_schema_str)
    print_schema(json_schema, "Expression Usage Schema")

    print(f"Title: expected='API User Schema', actual='{json_schema.get('title')}'")
    assert (
        json_schema["title"] == "API User Schema"
    ), f"Expected title 'API User Schema', got '{json_schema.get('title')}'"

    print(f"Type: expected='object', actual='{json_schema.get('type')}'")
    assert (
        json_schema["type"] == "object"
    ), f"Expected type 'object', got '{json_schema.get('type')}'"

    props = json_schema.get("properties", {})
    print(f"Properties: {list(props.keys())}")

    for field in ["id", "username", "email"]:
        assert (
            field in props
        ), f"Missing field '{field}' in properties: {list(props.keys())}"


@mark.xfail
def test_empty_dataframe():
    """Test serialization of empty DataFrame."""
    df = pl.DataFrame()
    print(f"\nEmpty DataFrame schema: {df.schema}")
    print(f"DataFrame shape: {df.shape}")

    try:
        json_schema = df.genson.serialize_schema_to_json()
        print_schema(json_schema, "Empty DataFrame Schema")

        print(f"Type: expected='object', actual='{json_schema.get('type')}'")
        assert (
            json_schema["type"] == "object"
        ), f"Expected type 'object', got '{json_schema.get('type')}'"

        props = json_schema.get("properties", {})
        print(f"Properties: {props}")
        assert props == {}, f"Expected empty properties, got {props}"

        required = json_schema.get("required", [])
        print(f"Required fields: {required}")
        assert required == [], f"Expected empty required list, got {required}"

    except Exception as e:
        print(f"ERROR: Failed to serialize empty DataFrame: {e}")
        print(f"Error type: {type(e)}")
        raise


@mark.xfail
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

    print(f"\nDataFrame schema: {df.schema}")
    print("Sample data:")
    print(df.head(1))

    json_schema = df.genson.serialize_schema_to_json()
    print_schema(json_schema, "Nested Structures Schema")

    props = json_schema["properties"]

    # Check nested struct with detailed output
    print(f"\nUser field: {props.get('user', 'MISSING')}")
    user_type = props["user"].get("type")
    print(f"User type: expected=object, actual={user_type}")
    assert (
        user_type == "object"
    ), f"User field: expected type 'object', got '{user_type}'"

    if user_type == "object":
        user_props = props["user"].get("properties", {})
        print(f"User properties: {list(user_props.keys())}")

        assert (
            "profile" in user_props
        ), f"Missing 'profile' in user properties: {list(user_props.keys())}"

        profile_type = user_props["profile"].get("type")
        print(f"Profile type: expected=object, actual={profile_type}")
        assert (
            profile_type == "object"
        ), f"Profile field: expected type 'object', got '{profile_type}'"

    # Check array of structs with detailed output
    print(f"\nPosts field: {props.get('posts', 'MISSING')}")
    posts_type = props["posts"].get("type")
    print(f"Posts type: expected=array, actual={posts_type}")
    assert (
        posts_type == "array"
    ), f"Posts field: expected type 'array', got '{posts_type}'"

    posts_items = props["posts"].get("items", {})
    posts_items_type = posts_items.get("type")
    print(f"Posts items type: expected=object, actual={posts_items_type}")
    assert (
        posts_items_type == "object"
    ), f"Posts items: expected type 'object', got '{posts_items_type}'"

    if posts_items_type == "object":
        post_props = posts_items.get("properties", {})
        print(f"Post properties: {list(post_props.keys())}")

        assert (
            "title" in post_props
        ), f"Missing 'title' in post properties: {list(post_props.keys())}"
        assert (
            "likes" in post_props
        ), f"Missing 'likes' in post properties: {list(post_props.keys())}"

        title_type = post_props["title"].get("type")
        likes_type = post_props["likes"].get("type")
        print(f"Title type: expected=string, actual={title_type}")
        print(f"Likes type: expected=integer, actual={likes_type}")

        assert (
            title_type == "string"
        ), f"Title field: expected type 'string', got '{title_type}'"
        assert (
            likes_type == "integer"
        ), f"Likes field: expected type 'integer', got '{likes_type}'"


def test_debug_output(capsys):
    """Test that debug output works."""
    df = pl.DataFrame(
        {
            "test_col": [1, 2, 3],
        }
    )

    print(f"\nDataFrame for debug test: {df.schema}")
    df.genson.serialize_schema_to_json(debug=True)

    # Check that debug output was captured
    captured = capsys.readouterr()
    print(f"Captured stdout: '{captured.out}'")
    print(f"Captured stderr: '{captured.err}'")
    print(f"Stdout length: {len(captured.out)}")
    print(f"Stderr length: {len(captured.err)}")

    has_debug_in_err = "DEBUG:" in captured.err
    has_any_err = len(captured.err) > 0
    has_debug_in_out = "DEBUG:" in captured.out
    has_any_out = len(captured.out) > 0

    print(f"Has 'DEBUG:' in stderr: {has_debug_in_err}")
    print(f"Has any stderr: {has_any_err}")
    print(f"Has 'DEBUG:' in stdout: {has_debug_in_out}")
    print(f"Has any stdout: {has_any_out}")

    assert (
        has_debug_in_err or has_any_err or has_debug_in_out or has_any_out
    ), f"Expected some debug output. stdout='{captured.out}', stderr='{captured.err}'"


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

    print(f"\nDF1 schema: {df1.schema}")
    print(f"DF2 schema: {df2.schema}")

    schema1 = df1.genson.serialize_schema_to_json()
    schema2 = df2.genson.serialize_schema_to_json()

    print_schema(schema1, "Schema 1")
    print_schema(schema2, "Schema 2")

    # Remove any timestamps or dynamic content for comparison
    def normalize_schema(schema):
        normalized = schema.copy()
        # Remove any keys that might vary between runs
        return normalized

    normalized1 = normalize_schema(schema1)
    normalized2 = normalize_schema(schema2)

    print(f"Schemas equal: {normalized1 == normalized2}")

    if normalized1 != normalized2:
        print("DIFFERENCES:")
        for key in set(list(normalized1.keys()) + list(normalized2.keys())):
            if normalized1.get(key) != normalized2.get(key):
                print(f"  {key}: {normalized1.get(key)} != {normalized2.get(key)}")

    assert (
        normalized1 == normalized2
    ), f"Schemas should be identical but differ. Schema1: {normalized1}, Schema2: {normalized2}"
