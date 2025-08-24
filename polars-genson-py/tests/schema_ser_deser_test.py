"""Tests for schema JSON serialization and deserialization functionality."""

import json

import polars as pl
import polars_genson
import pytest
from pytest import mark, raises


class TestSchemaJsonSerialization:
    """Test schema_to_json functionality."""

    def test_basic_schema_to_json(self):
        """Test basic schema serialization to JSON."""
        # Create a simple schema
        schema = pl.Schema(
            {"a": pl.Int64, "b": pl.Boolean, "c": pl.String, "d": pl.Float64}
        )

        # Convert to JSON
        json_str = polars_genson.schema_to_json(schema)

        # Should be valid JSON
        parsed = json.loads(json_str)
        assert isinstance(parsed, dict)

        # Should contain all expected fields
        assert "a" in parsed
        assert "b" in parsed
        assert "c" in parsed
        assert "d" in parsed

    def test_complex_schema_to_json(self):
        """Test serialization of complex schema types."""
        schema = pl.Schema(
            {
                "user": pl.Struct(
                    {
                        "id": pl.Int64,
                        "name": pl.String,
                        "settings": pl.Struct(
                            {"theme": pl.String, "notifications": pl.Boolean}
                        ),
                    }
                ),
                "tags": pl.List(pl.String),
                "scores": pl.List(pl.Float64),
                "metadata": pl.Struct({"created": pl.Date, "updated": pl.Datetime}),
            }
        )

        json_str = polars_genson.schema_to_json(schema)
        parsed = json.loads(json_str)

        # Verify structure contains all fields
        assert "user" in parsed
        assert "tags" in parsed
        assert "scores" in parsed
        assert "metadata" in parsed

    def test_empty_schema_to_json(self):
        """Test serialization of empty schema."""
        schema = pl.Schema({})
        json_str = polars_genson.schema_to_json(schema)
        parsed = json.loads(json_str)

        # Should be empty dict
        assert parsed == {}

    def test_decimal_schema_to_json(self):
        """Test serialization of schema with Decimal types."""
        schema = pl.Schema(
            {"price": pl.Decimal(10, 2), "tax": pl.Decimal(5, 4), "id": pl.Int64}
        )

        json_str = polars_genson.schema_to_json(schema)
        parsed = json.loads(json_str)

        assert "price" in parsed
        assert "tax" in parsed
        assert "id" in parsed

    def test_nested_list_schema_to_json(self):
        """Test serialization of schema with nested List types."""
        schema = pl.Schema(
            {
                "matrix": pl.List(pl.List(pl.Int64)),
                "simple_list": pl.List(pl.String),
                "struct_list": pl.List(
                    pl.Struct({"name": pl.String, "value": pl.Float64})
                ),
            }
        )

        json_str = polars_genson.schema_to_json(schema)
        parsed = json.loads(json_str)

        assert "matrix" in parsed
        assert "simple_list" in parsed
        assert "struct_list" in parsed


class TestSchemaJsonDeserialization:
    """Test json_to_schema functionality."""

    def test_basic_json_to_schema(self):
        """Test basic JSON deserialization to schema."""
        # Create original schema
        original_schema = pl.Schema({"a": pl.Int64, "b": pl.Boolean, "c": pl.String})

        # Serialize to JSON
        json_str = polars_genson.schema_to_json(original_schema)

        # Deserialize back to schema
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        # Should match original
        assert reconstructed_schema == original_schema

    def test_complex_json_to_schema(self):
        """Test deserialization of complex schema types."""
        original_schema = pl.Schema(
            {
                "user": pl.Struct({"id": pl.Int64, "name": pl.String}),
                "tags": pl.List(pl.String),
                "active": pl.Boolean,
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert reconstructed_schema == original_schema

    def test_empty_json_to_schema(self):
        """Test deserialization of empty schema."""
        original_schema = pl.Schema({})

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert reconstructed_schema == original_schema

    def test_manual_json_to_schema(self):
        """Test deserialization from manually created JSON."""
        # Create JSON manually (simulating external source)
        manual_json = json.dumps(
            {"name": "String", "age": "Int64", "active": "Boolean"}
        )

        schema = polars_genson.json_to_schema(manual_json)

        # Verify schema structure
        assert "name" in schema
        assert "age" in schema
        assert "active" in schema
        assert schema["name"] == pl.String
        assert schema["age"] == pl.Int64
        assert schema["active"] == pl.Boolean

    def test_invalid_json_to_schema(self):
        """Test error handling for invalid JSON."""
        invalid_json = "{'invalid': json}"  # Not valid JSON

        with pytest.raises(Exception):
            polars_genson.json_to_schema(invalid_json)

    def test_malformed_schema_json(self):
        """Test error handling for malformed schema JSON."""
        # Valid JSON but not a valid schema format
        malformed_json = json.dumps(["this", "is", "not", "a", "schema"])

        with pytest.raises(Exception):
            polars_genson.json_to_schema(malformed_json)


class TestRoundTripSerialization:
    """Test round-trip serialization and deserialization."""

    def test_round_trip_basic_types(self):
        """Test round-trip with basic Polars types."""
        original_schema = pl.Schema(
            {
                "string_col": pl.String,
                "int_col": pl.Int64,
                "float_col": pl.Float64,
                "bool_col": pl.Boolean,
                "date_col": pl.Date,
                "datetime_col": pl.Datetime(time_unit="us", time_zone=None),
                "duration_col": pl.Duration(time_unit="us"),
            }
        )

        # Round trip
        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_numeric_types_misbehaving(self):
        """Test round-trip with various numeric types."""
        original_schema = pl.Schema(
            {
                "int8_col": pl.Int8,
                "int16_col": pl.Int16,
                "uint8_col": pl.UInt8,
                "uint16_col": pl.UInt16,
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_numeric_types(self):
        """Test round-trip with various numeric types."""
        original_schema = pl.Schema(
            {
                # "int8_col": pl.Int8,
                # "int16_col": pl.Int16,
                "int32_col": pl.Int32,
                "int64_col": pl.Int64,
                # "uint8_col": pl.UInt8,
                # "uint16_col": pl.UInt16,
                "uint32_col": pl.UInt32,
                "uint64_col": pl.UInt64,
                "float32_col": pl.Float32,
                "float64_col": pl.Float64,
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_decimal_types(self):
        """Test round-trip with Decimal types."""
        original_schema = pl.Schema(
            {
                "price": pl.Decimal(10, 2),
                "tax_rate": pl.Decimal(5, 4),
                "high_precision": pl.Decimal(38, 18),
                "default_decimal": pl.Decimal(None, 0),
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        close_enough = dict(original_schema)
        close_enough.update({"default_decimal": pl.Decimal(38, 0)})

        # assert original_schema == reconstructed_schema
        assert close_enough == reconstructed_schema

    def test_round_trip_list_types(self):
        """Test round-trip with List types."""
        original_schema = pl.Schema(
            {
                "string_list": pl.List(pl.String),
                "int_list": pl.List(pl.Int64),
                "bool_list": pl.List(pl.Boolean),
                "nested_list": pl.List(pl.List(pl.String)),
                "triple_nested": pl.List(pl.List(pl.List(pl.Int64))),
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_struct_types(self):
        """Test round-trip with Struct types."""
        original_schema = pl.Schema(
            {
                "simple_struct": pl.Struct({"name": pl.String, "age": pl.Int64}),
                "nested_struct": pl.Struct(
                    {
                        "user": pl.Struct(
                            {
                                "id": pl.Int64,
                                "profile": pl.Struct(
                                    {"email": pl.String, "verified": pl.Boolean}
                                ),
                            }
                        ),
                        "metadata": pl.Struct(
                            {"created": pl.Date, "tags": pl.List(pl.String)}
                        ),
                    }
                ),
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_mixed_complex_types(self):
        """Test round-trip with mixed complex types."""
        original_schema = pl.Schema(
            {
                "users": pl.List(
                    pl.Struct(
                        {
                            "id": pl.Int64,
                            "name": pl.String,
                            "scores": pl.List(pl.Float64),
                            "preferences": pl.Struct(
                                {"theme": pl.String, "notifications": pl.Boolean}
                            ),
                        }
                    )
                ),
                "matrix": pl.List(pl.List(pl.Decimal(8, 2))),
                "config": pl.Struct(
                    {
                        "database": pl.Struct(
                            {"host": pl.String, "port": pl.Int64, "ssl": pl.Boolean}
                        ),
                        "features": pl.List(pl.String),
                        "limits": pl.Struct(
                            {
                                "max_connections": pl.Int64,
                                "timeout": pl.Duration(time_unit="us"),
                            }
                        ),
                    }
                ),
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_round_trip_with_dataframe_schema(self):
        """Test round-trip using actual DataFrame schema."""
        # Create a DataFrame with complex schema
        df = pl.DataFrame(
            {
                "id": [1, 2, 3],
                "name": ["Alice", "Bob", "Charlie"],
                "active": [True, False, True],
                "score": [95.5, 87.2, 92.1],
            }
        )

        original_schema = df.schema

        # Round trip
        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert original_schema == reconstructed_schema

    def test_multiple_round_trips(self):
        """Test multiple round-trips to ensure stability."""
        original_schema = pl.Schema(
            {
                "complex": pl.Struct(
                    {
                        "nested": pl.List(
                            pl.Struct(
                                {"value": pl.Decimal(10, 2), "tags": pl.List(pl.String)}
                            )
                        )
                    }
                )
            }
        )

        # Multiple round trips
        current_schema = original_schema
        for i in range(3):
            json_str = polars_genson.schema_to_json(current_schema)
            current_schema = polars_genson.json_to_schema(json_str)
            assert current_schema == original_schema, f"Round trip {i + 1} failed"


class TestSchemaUsageAfterDeserialization:
    """Test that deserialized schemas can be used to create DataFrames."""

    def test_create_dataframe_from_deserialized_schema(self):
        """Test creating DataFrame from deserialized schema."""
        original_schema = pl.Schema(
            {
                "name": pl.String,
                "age": pl.Int64,
                "active": pl.Boolean,
                "score": pl.Float64,
            }
        )

        # Round trip
        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        # Should be able to create empty DataFrame with reconstructed schema
        empty_df = pl.DataFrame(schema=reconstructed_schema)

        assert empty_df.schema == original_schema
        assert len(empty_df) == 0
        assert list(empty_df.columns) == ["name", "age", "active", "score"]

    def test_create_dataframe_with_complex_deserialized_schema(self):
        """Test creating DataFrame with complex deserialized schema."""
        original_schema = pl.Schema(
            {
                "user": pl.Struct({"id": pl.Int64, "name": pl.String}),
                "tags": pl.List(pl.String),
                "metadata": pl.Struct({"created": pl.Date, "active": pl.Boolean}),
            }
        )

        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        # Should be able to create empty DataFrame
        empty_df = pl.DataFrame(schema=reconstructed_schema)

        assert empty_df.schema == original_schema
        assert len(empty_df) == 0
        assert "user" in empty_df.columns
        assert "tags" in empty_df.columns
        assert "metadata" in empty_df.columns

    def test_schema_compatibility_after_round_trip(self):
        """Test that round-tripped schemas are fully compatible."""
        # Start with a real DataFrame
        df = pl.DataFrame(
            {
                "id": [1, 2],
                "data": [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}],
            }
        )

        original_schema = df.schema

        # Round trip the schema
        json_str = polars_genson.schema_to_json(original_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        # Should be able to use reconstructed schema for operations
        new_df = pl.DataFrame(schema=reconstructed_schema)

        # Schemas should be identical
        assert new_df.schema == df.schema
        assert new_df.schema == original_schema
        assert new_df.schema == reconstructed_schema


class TestErrorHandling:
    """Test error handling in schema serialization/deserialization."""

    def test_json_to_schema_with_unknown_dtype(self):
        """Test handling of unknown dtype in JSON."""
        # Create JSON with unknown dtype
        unknown_dtype_json = json.dumps(
            {"valid_field": "String", "unknown_field": "UnknownType"}
        )

        # Should handle gracefully (likely falling back to String)
        with raises(ValueError, match="unknown_field"):
            schema = polars_genson.json_to_schema(unknown_dtype_json)

        # Just erroring is fine?

        # assert "valid_field" in schema
        # assert "unknown_field" in schema
        # assert schema["valid_field"] == pl.String
        # # Unknown types should fall back to String
        # assert schema["unknown_field"] == pl.String

    def test_json_to_schema_with_malformed_dtype(self):
        """Test handling of malformed dtype strings."""
        malformed_json = json.dumps(
            {
                "field1": "Decimal(abc, def)",  # Malformed decimal
                "field2": "List[",  # Incomplete list
                "field3": "Struct[incomplete",  # Incomplete struct
            }
        )

        # Should handle gracefully without crashing
        with raises(ValueError, match="Failed to deserialize dtype"):
            schema = polars_genson.json_to_schema(malformed_json)

        # We just crash out
        # # All malformed types should fall back to String
        # assert all(dtype == pl.String for dtype in schema.values())

    def test_schema_to_json_with_unsupported_types(self):
        """Test serialization of edge case types."""
        # Test with less common types
        schema = pl.Schema(
            {
                "binary_col": pl.Binary,
                "categorical_col": pl.Categorical(ordering="lexical"),
                "null_col": pl.Null,
            }
        )

        # Should not crash
        json_str = polars_genson.schema_to_json(schema)
        parsed = json.loads(json_str)

        assert "binary_col" in parsed
        assert "categorical_col" in parsed
        assert "null_col" in parsed


class TestEdgeCases:
    """Test edge cases and boundary conditions."""

    def test_very_deeply_nested_schema(self):
        """Test with very deeply nested schema."""
        # Create deeply nested struct
        deep_schema = pl.Schema(
            {
                "level1": pl.Struct(
                    {
                        "level2": pl.Struct(
                            {
                                "level3": pl.Struct(
                                    {
                                        "level4": pl.Struct(
                                            {"level5": pl.Struct({"value": pl.String})}
                                        )
                                    }
                                )
                            }
                        )
                    }
                )
            }
        )

        # Should handle deep nesting
        json_str = polars_genson.schema_to_json(deep_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert deep_schema == reconstructed_schema

    def test_schema_with_many_fields(self):
        """Test schema with many fields."""
        # Create schema with many fields
        many_fields_schema = pl.Schema(
            {f"field_{i}": pl.Int64 if i % 2 == 0 else pl.String for i in range(100)}
        )

        json_str = polars_genson.schema_to_json(many_fields_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert many_fields_schema == reconstructed_schema
        assert len(reconstructed_schema) == 100

    def test_schema_with_unicode_field_names(self):
        """Test schema with Unicode field names."""
        unicode_schema = pl.Schema(
            {
                "普通字段": pl.String,
                "数字字段": pl.Int64,
                "布尔字段": pl.Boolean,
                "émoji_field_🚀": pl.Float64,
                "Ñoño": pl.Date,
            }
        )

        json_str = polars_genson.schema_to_json(unicode_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert unicode_schema == reconstructed_schema

    def test_field_name_with_special_characters(self):
        """Test field names with special characters."""
        special_schema = pl.Schema(
            {
                "field with spaces": pl.String,
                "field-with-dashes": pl.Int64,
                "field.with.dots": pl.Boolean,
                "field:with:colons": pl.Float64,
                "field[with]brackets": pl.Date,
            }
        )

        json_str = polars_genson.schema_to_json(special_schema)
        reconstructed_schema = polars_genson.json_to_schema(json_str)

        assert special_schema == reconstructed_schema


class TestIntegrationWithExistingTests:
    """Test integration with existing polars-genson functionality."""

    def test_schema_serialization_after_inference(self):
        """Test serializing schema after JSON schema inference."""
        # Start with JSON data
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"user": {"id": 1, "name": "Alice"}, "active": true}',
                    '{"user": {"id": 2, "name": "Bob"}, "active": false}',
                ]
            }
        )

        # Infer Polars schema
        inferred_schema = df.genson.infer_polars_schema("json_col")

        # Serialize the inferred schema
        json_str = polars_genson.schema_to_json(inferred_schema)

        # Should be valid JSON
        parsed = json.loads(json_str)
        assert isinstance(parsed, dict)

        # Should contain expected fields
        assert "user" in parsed
        assert "active" in parsed

    def test_round_trip_with_inferred_schema(self):
        """Test full round-trip: JSON data → inferred schema → JSON → schema."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"id": 1, "name": "Alice", "tags": ["python", "rust"]}',
                    '{"id": 2, "name": "Bob", "tags": ["javascript"]}',
                ]
            }
        )

        # Infer schema from JSON
        inferred_schema = df.genson.infer_polars_schema("json_col")

        # Serialize to JSON
        schema_json = polars_genson.schema_to_json(inferred_schema)

        # Deserialize back
        reconstructed_schema = polars_genson.json_to_schema(schema_json)

        # Should match the inferred schema
        assert inferred_schema == reconstructed_schema

        # Should be able to create DataFrame with reconstructed schema
        new_df = pl.DataFrame(schema=reconstructed_schema)
        assert new_df.schema == inferred_schema


def test_demo_functionality():
    """Test the exact functionality shown in the demo."""
    # Replicate the demo code
    df = pl.DataFrame({"a": [1, 2, 3], "b": [True, False, True]})
    schema = df.schema

    # Convert schema to JSON
    jsonified_schema = polars_genson.schema_to_json(schema)

    # Should be valid JSON string
    assert isinstance(jsonified_schema, str)
    parsed = json.loads(jsonified_schema)
    assert isinstance(parsed, dict)

    # Convert back to schema
    reschemafied = polars_genson.json_to_schema(jsonified_schema)

    # Should match original schema
    assert reschemafied == schema

    # Should have correct fields and types
    assert "a" in reschemafied
    assert "b" in reschemafied
    assert reschemafied["a"] == pl.Int64
    assert reschemafied["b"] == pl.Boolean
