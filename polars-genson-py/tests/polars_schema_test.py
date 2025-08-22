"""Test the production of Polars schemas via schema inference."""

import polars as pl
import pytest
from polars_genson import infer_polars_schema
from polars_genson.dtypes import _parse_polars_dtype


# Unit tests for _parse_polars_dtype function
class TestDtypeParsing:
    """Unit tests for dtype string parsing."""

    def test_simple_types(self):
        """Test parsing of simple scalar types."""
        assert _parse_polars_dtype("String") == pl.Utf8
        assert _parse_polars_dtype("Int64") == pl.Int64
        assert _parse_polars_dtype("Float64") == pl.Float64
        assert _parse_polars_dtype("Boolean") == pl.Boolean

    def test_decimal_types(self):
        """Test parsing of Decimal types with and without parameters."""
        assert _parse_polars_dtype("Decimal(38, 9)") == pl.Decimal(38, 9)
        assert _parse_polars_dtype("Decimal(10, 2)") == pl.Decimal(10, 2)
        assert _parse_polars_dtype("Decimal") == pl.Decimal(None, None)

        # Test with extra whitespace
        assert _parse_polars_dtype("Decimal( 38 , 9 )") == pl.Decimal(38, 9)

    def test_list_types(self):
        """Test parsing of List types."""
        assert _parse_polars_dtype("List[String]") == pl.List(pl.Utf8)
        assert _parse_polars_dtype("List[Int64]") == pl.List(pl.Int64)
        assert _parse_polars_dtype("List[Boolean]") == pl.List(pl.Boolean)

    def test_simple_struct_types(self):
        """Test parsing of simple Struct types."""
        struct_type = _parse_polars_dtype("Struct[name:String,age:Int64]")
        assert isinstance(struct_type, pl.Struct)

        # Convert to dict for easier comparison
        fields_dict = {field.name: field.dtype for field in struct_type.fields}
        assert fields_dict == {"name": pl.Utf8, "age": pl.Int64}

    def test_nested_struct_types(self):
        """Test parsing of nested Struct types."""
        nested_struct = _parse_polars_dtype(
            "Struct[user:Struct[id:Int64,name:String],active:Boolean]"
        )
        assert isinstance(nested_struct, pl.Struct)

        fields_dict = {field.name: field.dtype for field in nested_struct.fields}
        assert "user" in fields_dict
        assert "active" in fields_dict
        assert fields_dict["active"] == pl.Boolean

        # Check the nested struct
        user_struct = fields_dict["user"]
        assert isinstance(user_struct, pl.Struct)
        user_fields = {field.name: field.dtype for field in user_struct.fields}
        assert user_fields == {"id": pl.Int64, "name": pl.Utf8}

    def test_struct_with_decimal(self):
        """Test parsing of Struct containing Decimal fields."""
        struct_type = _parse_polars_dtype("Struct[price:Decimal(10,2),quantity:Int64]")
        assert isinstance(struct_type, pl.Struct)

        fields_dict = {field.name: field.dtype for field in struct_type.fields}
        assert fields_dict["price"] == pl.Decimal(10, 2)
        assert fields_dict["quantity"] == pl.Int64

    def test_struct_with_lists(self):
        """Test parsing of Struct containing List fields."""
        struct_type = _parse_polars_dtype(
            "Struct[tags:List[String],scores:List[Int64]]"
        )
        assert isinstance(struct_type, pl.Struct)

        fields_dict = {field.name: field.dtype for field in struct_type.fields}
        assert fields_dict["tags"] == pl.List(pl.Utf8)
        assert fields_dict["scores"] == pl.List(pl.Int64)

    def test_empty_struct(self):
        """Test parsing of empty Struct."""
        struct_type = _parse_polars_dtype("Struct[]")
        assert isinstance(struct_type, pl.Struct)
        assert len(struct_type.fields) == 0


# Integration tests for the full infer_polars_schema pipeline
class TestPolarsSchemaInference:
    """Integration tests for Polars schema inference."""

    def test_basic_schema_inference(self):
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

    def test_mixed_types(self):
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

    def test_nested_objects(self):
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

    def test_arrays(self):
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

    def test_complex_nested_structure(self):
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
                            {
                                "name": pl.String,
                                "settings": pl.Struct({"theme": pl.String}),
                            }
                        )
                    }
                ),
                "posts": pl.List(pl.Struct({"title": pl.String, "likes": pl.Int64})),
            }
        )

    def test_optional_fields(self):
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

    def test_mixed_array_types(self):
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

    def test_empty_objects_and_arrays(self):
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

    def test_decimal_fields_in_schema(self):
        """Test schema inference with decimal numbers that should map to Decimal types."""
        # Note: This test assumes your Rust side infers Decimal types for high-precision numbers
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"id": 1, "price": 123.45, "precise_value": 999999999.123456789}',
                    '{"id": 2, "price": 67.89, "precise_value": 888888888.987654321}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        # At minimum, we should be able to handle the schema if it contains Decimal types
        # The exact mapping depends on your Rust implementation
        assert "id" in schema
        assert "price" in schema
        assert "precise_value" in schema

    def test_schema_with_struct_containing_decimals(self):
        """Test that schemas with Structs containing Decimal fields parse correctly."""
        # This would test the case where your Rust side returns something like:
        # Struct[product:Struct[price:Decimal(10,2),tax:Decimal(5,4)]]
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"product": {"name": "Widget", "price": 19.99}}',
                    '{"product": {"name": "Gadget", "price": 29.99}}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        # Verify the structure is parsed correctly
        assert "product" in schema
        product_type = schema["product"]
        assert isinstance(product_type, pl.Struct)

    def test_schema_consistency(self):
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

    def test_single_row(self):
        """Test schema inference with just one row."""
        df = pl.DataFrame(
            {"json_col": ['{"single": {"nested": {"value": [1, 2, 3]}}}']}
        )

        schema = df.genson.infer_polars_schema("json_col")

        assert schema == pl.Schema(
            {
                "single": pl.Struct(
                    {"nested": pl.Struct({"value": pl.List(pl.Int64)})}
                ),
            }
        )

    def test_deeply_nested_struct_with_mixed_types(self):
        """Test complex nested structures that would stress-test the dtype parser."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"config": {"database": {"host": "localhost", "port": 5432, "ssl": true}, "features": ["auth", "logging"]}}',
                    '{"config": {"database": {"host": "prod.db", "port": 3306, "ssl": false}, "features": ["auth"]}}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        expected = pl.Schema(
            {
                "config": pl.Struct(
                    {
                        "database": pl.Struct(
                            {
                                "host": pl.String,
                                "port": pl.Int64,
                                "ssl": pl.Boolean,
                            }
                        ),
                        "features": pl.List(pl.String),
                    }
                )
            }
        )

        assert schema == expected
