"""Tests for merge_schemas parameter."""

import orjson
import polars as pl
import polars_genson  # noqa: F401


def test_merge_schemas_true():
    """Test merge_schemas=True (default behavior)."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}',
                '{"name": "Bob", "age": 25, "city": "NYC"}',
                '{"name": "Charlie", "age": 35, "email": "charlie@example.com"}',
            ]
        }
    )

    schema = df.genson.infer_json_schema("json_data", merge_schemas=True)

    # Should return a single merged schema
    assert isinstance(schema, dict)
    assert "properties" in schema

    props = schema["properties"]
    # Should contain all properties from all JSON objects
    assert "name" in props
    assert "age" in props
    assert "city" in props
    assert "email" in props


def test_merge_schemas_false():
    """Test merge_schemas=False (individual schemas)."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"user": {"name": "Alice", "age": 30}}',
                '{"product": {"id": 123, "price": 29.99}}',
                '{"event": {"timestamp": "2024-01-01", "type": "click"}}',
            ]
        }
    )

    schemas = df.genson.infer_json_schema("json_data", merge_schemas=False)

    # Should return a list of individual schemas
    assert isinstance(schemas, list)
    assert len(schemas) == 3

    # Each should be a valid schema
    for schema in schemas:
        assert isinstance(schema, dict)
        assert "type" in schema
        assert "properties" in schema

    # Schemas should be different from each other
    schema_strs = [orjson.dumps(s, option=orjson.OPT_SORT_KEYS) for s in schemas]
    assert len(set(schema_strs)) == 3  # All unique


def test_concordant_vs_discordant():
    """Test the difference between similar and different schemas."""
    # Concordant schemas (similar structure)
    concordant_df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}',
                '{"name": "Bob", "age": 25, "city": "NYC"}',
                '{"name": "Charlie", "age": 35, "email": "charlie@example.com"}',
            ]
        }
    )

    # Discordant schemas (completely different)
    discordant_df = pl.DataFrame(
        {
            "json_data": [
                '{"user_profile": {"name": "Alice", "preferences": {"theme": "dark"}}}',
                '{"order_details": {"order_id": "12345", "items": [{"sku": "ABC", "qty": 2}]}}',
                '{"system_metrics": {"cpu_usage": 75.5, "memory_gb": 8, "timestamp": 1640995200}}',
            ]
        }
    )

    # Test concordant schemas
    concordant_merged = concordant_df.genson.infer_json_schema(
        "json_data", merge_schemas=True
    )
    concordant_individual = concordant_df.genson.infer_json_schema(
        "json_data", merge_schemas=False
    )

    # For concordant schemas, merged should make sense
    assert "name" in concordant_merged["properties"]
    assert "age" in concordant_merged["properties"]

    # Individual schemas should still be similar
    assert len(concordant_individual) == 3

    # Test discordant schemas
    discordant_merged = discordant_df.genson.infer_json_schema(
        "json_data", merge_schemas=True
    )
    discordant_individual = discordant_df.genson.infer_json_schema(
        "json_data", merge_schemas=False
    )

    # For discordant schemas, individual might be more meaningful
    assert len(discordant_individual) == 3

    # Each individual schema should be quite different
    individual_props = [set(s["properties"].keys()) for s in discordant_individual]
    assert individual_props[0] != individual_props[1]
    assert individual_props[1] != individual_props[2]


def test_expression_with_merge_schemas():
    """Test using merge_schemas parameter with direct expression."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"type": "user", "data": {"name": "Alice"}}',
                '{"type": "order", "data": {"id": 123, "total": 99.99}}',
            ]
        }
    )

    # Test merged
    merged_result = df.select(
        polars_genson.infer_json_schema(pl.col("json_data"), merge_schemas=True).alias(
            "schema"
        )
    )
    merged_schema = orjson.loads(merged_result.get_column("schema").first())

    # Test individual
    individual_result = df.select(
        polars_genson.infer_json_schema(pl.col("json_data"), merge_schemas=False).alias(
            "schemas"
        )
    )
    individual_schemas = orjson.loads(individual_result.get_column("schemas").first())

    # Verify types
    assert isinstance(merged_schema, dict)
    assert isinstance(individual_schemas, list)
    assert len(individual_schemas) == 2
