"""Comprehensive tests for dtype string parsing."""

import pytest
import polars as pl
from polars_genson.dtypes import _parse_polars_dtype, _split_struct_fields


class TestDtypeParsing:
    """Test the _parse_polars_dtype function comprehensively."""

    def test_simple_scalar_types(self):
        """Test all basic scalar type mappings."""
        test_cases = [
            ("String", pl.Utf8),
            ("Int64", pl.Int64),
            ("Int32", pl.Int32),
            ("Int16", pl.Int16),
            ("Int8", pl.Int8),
            ("UInt64", pl.UInt64),
            ("UInt32", pl.UInt32),
            ("UInt16", pl.UInt16),
            ("UInt8", pl.UInt8),
            ("Float64", pl.Float64),
            ("Float32", pl.Float32),
            ("Boolean", pl.Boolean),
            ("Date", pl.Date),
            ("Time", pl.Time),
            ("Datetime", pl.Datetime),
            ("Duration", pl.Duration),
            ("Null", pl.Null),
            ("Binary", pl.Binary),
            ("Categorical", pl.Categorical),
        ]

        for dtype_str, expected in test_cases:
            result = _parse_polars_dtype(dtype_str)
            assert result == expected, (
                f"Failed for {dtype_str}: got {result}, expected {expected}"
            )

    def test_decimal_parsing(self):
        """Test Decimal type parsing with various formats."""
        # Standard format
        assert _parse_polars_dtype("Decimal(38, 9)") == pl.Decimal(38, 9)
        assert _parse_polars_dtype("Decimal(10, 2)") == pl.Decimal(10, 2)
        assert _parse_polars_dtype("Decimal(5, 0)") == pl.Decimal(5, 0)

        # With whitespace
        assert _parse_polars_dtype("Decimal( 38 , 9 )") == pl.Decimal(38, 9)
        assert _parse_polars_dtype("Decimal(  10,   2  )") == pl.Decimal(10, 2)

        # Without parameters
        assert _parse_polars_dtype("Decimal") == pl.Decimal(None, None)

        # Edge case: malformed should fall back to default
        assert _parse_polars_dtype("Decimal(abc, def)") == pl.Decimal(None, None)

    def test_list_parsing(self):
        """Test List type parsing."""
        assert _parse_polars_dtype("List[String]") == pl.List(pl.Utf8)
        assert _parse_polars_dtype("List[Int64]") == pl.List(pl.Int64)
        assert _parse_polars_dtype("List[Boolean]") == pl.List(pl.Boolean)
        assert _parse_polars_dtype("List[Float64]") == pl.List(pl.Float64)

    def test_nested_list_parsing(self):
        """Test nested List type parsing."""
        # List of Lists
        result = _parse_polars_dtype("List[List[String]]")
        assert isinstance(result, pl.List)
        assert isinstance(result.inner, pl.List)
        assert result.inner.inner == pl.Utf8

    def test_array_parsing(self):
        """Test Array type parsing."""
        assert _parse_polars_dtype("Array[Int64,5]") == pl.Array(pl.Int64, 5)
        assert _parse_polars_dtype("Array[String,10]") == pl.Array(pl.Utf8, 10)

        # With whitespace
        assert _parse_polars_dtype("Array[Int64, 5]") == pl.Array(pl.Int64, 5)
        assert _parse_polars_dtype("Array[ String , 10 ]") == pl.Array(pl.Utf8, 10)

    def test_simple_struct_parsing(self):
        """Test simple Struct type parsing."""
        result = _parse_polars_dtype("Struct[name:String,age:Int64]")
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}
        assert fields == {"name": pl.Utf8, "age": pl.Int64}

    def test_struct_with_spaces(self):
        """Test Struct parsing with various whitespace."""
        result = _parse_polars_dtype("Struct[ name : String , age : Int64 ]")
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}
        assert fields == {"name": pl.Utf8, "age": pl.Int64}

    def test_empty_struct(self):
        """Test empty Struct parsing."""
        result = _parse_polars_dtype("Struct[]")
        assert isinstance(result, pl.Struct)
        assert len(result.fields) == 0

    def test_nested_struct_parsing(self):
        """Test nested Struct parsing."""
        dtype_str = "Struct[user:Struct[id:Int64,name:String],active:Boolean]"
        result = _parse_polars_dtype(dtype_str)
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}
        assert "user" in fields
        assert "active" in fields
        assert fields["active"] == pl.Boolean

        # Check nested struct
        user_struct = fields["user"]
        assert isinstance(user_struct, pl.Struct)
        user_fields = {field.name: field.dtype for field in user_struct.fields}
        assert user_fields == {"id": pl.Int64, "name": pl.Utf8}

    def test_struct_with_lists(self):
        """Test Struct containing List fields."""
        result = _parse_polars_dtype("Struct[tags:List[String],scores:List[Int64]]")
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}
        assert fields["tags"] == pl.List(pl.Utf8)
        assert fields["scores"] == pl.List(pl.Int64)

    def test_struct_with_decimals(self):
        """Test Struct containing Decimal fields."""
        result = _parse_polars_dtype("Struct[price:Decimal(10,2),tax:Decimal(5,4)]")
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}
        assert fields["price"] == pl.Decimal(10, 2)
        assert fields["tax"] == pl.Decimal(5, 4)

    def test_deeply_nested_structure(self):
        """Test very deeply nested structures."""
        dtype_str = "Struct[config:Struct[db:Struct[host:String,port:Int64],features:List[String]]]"
        result = _parse_polars_dtype(dtype_str)
        assert isinstance(result, pl.Struct)

        # Navigate through the nested structure
        config_field = next(f for f in result.fields if f.name == "config")
        assert isinstance(config_field.dtype, pl.Struct)

        db_field = next(f for f in config_field.dtype.fields if f.name == "db")
        assert isinstance(db_field.dtype, pl.Struct)

        db_fields = {f.name: f.dtype for f in db_field.dtype.fields}
        assert db_fields == {"host": pl.Utf8, "port": pl.Int64}

        features_field = next(
            f for f in config_field.dtype.fields if f.name == "features"
        )
        assert features_field.dtype == pl.List(pl.Utf8)

    def test_mixed_complex_types(self):
        """Test mixing various complex types."""
        dtype_str = "Struct[user:Struct[id:Int64,tags:List[String]],prices:List[Decimal(10,2)],active:Boolean]"
        result = _parse_polars_dtype(dtype_str)
        assert isinstance(result, pl.Struct)

        fields = {field.name: field.dtype for field in result.fields}

        # Check user struct
        assert isinstance(fields["user"], pl.Struct)
        user_fields = {f.name: f.dtype for f in fields["user"].fields}
        assert user_fields["id"] == pl.Int64
        assert user_fields["tags"] == pl.List(pl.Utf8)

        # Check prices list
        assert fields["prices"] == pl.List(pl.Decimal(10, 2))

        # Check boolean
        assert fields["active"] == pl.Boolean

    def test_unknown_type_fallback(self):
        """Test that unknown types fall back to String."""
        result = _parse_polars_dtype("UnknownType")
        assert result == pl.Utf8

    def test_whitespace_handling(self):
        """Test that whitespace is properly handled."""
        assert _parse_polars_dtype("  String  ") == pl.Utf8
        assert _parse_polars_dtype("\tInt64\n") == pl.Int64


class TestStructFieldSplitting:
    """Test the _split_struct_fields helper function."""

    def test_simple_fields(self):
        """Test splitting simple field definitions."""
        result = _split_struct_fields("name:String,age:Int64")
        assert result == ["name:String", "age:Int64"]

    def test_fields_with_spaces(self):
        """Test splitting with various whitespace."""
        result = _split_struct_fields("name : String , age : Int64")
        assert result == ["name : String", "age : Int64"]

    def test_nested_struct_fields(self):
        """Test splitting fields with nested structures."""
        result = _split_struct_fields(
            "user:Struct[id:Int64,name:String],active:Boolean"
        )
        assert result == ["user:Struct[id:Int64,name:String]", "active:Boolean"]

    def test_fields_with_lists(self):
        """Test splitting fields containing Lists."""
        result = _split_struct_fields("tags:List[String],scores:List[Int64]")
        assert result == ["tags:List[String]", "scores:List[Int64]"]

    def test_fields_with_decimals(self):
        """Test splitting fields containing Decimals with parentheses."""
        result = _split_struct_fields("price:Decimal(10,2),tax:Decimal(5,4)")
        assert result == ["price:Decimal(10,2)", "tax:Decimal(5,4)"]

    def test_complex_nested_fields(self):
        """Test splitting complex nested field definitions."""
        fields_str = "config:Struct[db:Struct[host:String,port:Int64],cache:Boolean],data:List[Decimal(10,2)]"
        result = _split_struct_fields(fields_str)
        expected = [
            "config:Struct[db:Struct[host:String,port:Int64],cache:Boolean]",
            "data:List[Decimal(10,2)]",
        ]
        assert result == expected

    def test_empty_fields(self):
        """Test splitting empty field string."""
        result = _split_struct_fields("")
        assert result == []

    def test_single_field(self):
        """Test splitting single field."""
        result = _split_struct_fields("name:String")
        assert result == ["name:String"]

    def test_deeply_nested_brackets(self):
        """Test splitting with deeply nested bracket structures."""
        fields_str = (
            "deep:Struct[level1:Struct[level2:Struct[value:Int64]]],simple:String"
        )
        result = _split_struct_fields(fields_str)
        expected = [
            "deep:Struct[level1:Struct[level2:Struct[value:Int64]]]",
            "simple:String",
        ]
        assert result == expected

    def test_mixed_parentheses_and_brackets(self):
        """Test splitting with both parentheses and brackets."""
        fields_str = (
            "prices:List[Decimal(10,2)],config:Struct[timeout:Duration],active:Boolean"
        )
        result = _split_struct_fields(fields_str)
        expected = [
            "prices:List[Decimal(10,2)]",
            "config:Struct[timeout:Duration]",
            "active:Boolean",
        ]
        assert result == expected


class TestIntegrationWithPolarsSchemas:
    """Test that the dtype parsing integrates correctly with the schema inference."""

    def test_schema_creation_with_structs(self):
        """Test that parsed Struct types work correctly in Schema creation."""
        struct_type = _parse_polars_dtype("Struct[name:String,age:Int64]")

        # Should be able to create a schema with this struct
        schema = pl.Schema({"user": struct_type, "active": pl.Boolean})

        assert "user" in schema
        assert "active" in schema
        assert schema["active"] == pl.Boolean
        assert isinstance(schema["user"], pl.Struct)

    def test_schema_creation_with_decimals(self):
        """Test that parsed Decimal types work correctly in Schema creation."""
        decimal_type = _parse_polars_dtype("Decimal(10,2)")

        # Should be able to create a schema with this decimal
        schema = pl.Schema({"price": decimal_type, "id": pl.Int64})

        assert schema["price"] == pl.Decimal(10, 2)
        assert schema["id"] == pl.Int64

    def test_complex_schema_composition(self):
        """Test creating complex schemas with mixed parsed types."""
        user_struct = _parse_polars_dtype(
            "Struct[id:Int64,profile:Struct[name:String,email:String]]"
        )
        prices_list = _parse_polars_dtype("List[Decimal(10,2)]")
        tags_list = _parse_polars_dtype("List[String]")

        schema = pl.Schema(
            {
                "user": user_struct,
                "prices": prices_list,
                "tags": tags_list,
                "active": pl.Boolean,
            }
        )

        # Verify the schema structure
        assert isinstance(schema["user"], pl.Struct)
        assert schema["prices"] == pl.List(pl.Decimal(10, 2))
        assert schema["tags"] == pl.List(pl.Utf8)
        assert schema["active"] == pl.Boolean

    def test_schema_field_access(self):
        """Test that we can access fields from parsed Struct types."""
        struct_type = _parse_polars_dtype(
            "Struct[user:Struct[id:Int64,name:String],count:Int64]"
        )

        # Should be able to access nested fields
        assert len(struct_type.fields) == 2

        field_names = [f.name for f in struct_type.fields]
        assert "user" in field_names
        assert "count" in field_names

        # Check nested structure
        user_field = next(f for f in struct_type.fields if f.name == "user")
        assert isinstance(user_field.dtype, pl.Struct)

        nested_field_names = [f.name for f in user_field.dtype.fields]
        assert "id" in nested_field_names
        assert "name" in nested_field_names
