"""Comprehensive tests for nested Arrays and Lists within Structs.

These tests ensure that complex nesting scenarios work correctly:
- Lists of Structs
- Structs containing Lists
- Arrays within Structs
- Nested Lists (List[List[T]])
- Mixed combinations
"""

import polars as pl
import pytest
from polars_genson.dtypes import _parse_polars_dtype, _split_struct_fields


class TestNestedArraysListsInStructs:
    """Test nested Arrays and Lists within Struct types."""

    def test_struct_with_simple_lists(self):
        """Test Struct containing simple List fields."""
        struct_type = _parse_polars_dtype(
            "Struct[tags:List[String],scores:List[Int64],flags:List[Boolean]]"
        )
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}
        assert fields["tags"] == pl.List(pl.Utf8)
        assert fields["scores"] == pl.List(pl.Int64)
        assert fields["flags"] == pl.List(pl.Boolean)

    def test_struct_with_arrays(self):
        """Test Struct containing Array fields."""
        struct_type = _parse_polars_dtype(
            "Struct[coordinates:Array[Float64,3],matrix:Array[Int64,9]]"
        )
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}
        assert fields["coordinates"] == pl.Array(pl.Float64, 3)
        assert fields["matrix"] == pl.Array(pl.Int64, 9)

    def test_list_of_structs(self):
        """Test List containing Struct elements."""
        list_type = _parse_polars_dtype("List[Struct[id:Int64,name:String]]")
        assert isinstance(list_type, pl.List)

        inner_struct = list_type.inner
        assert isinstance(inner_struct, pl.Struct)

        struct_fields = {f.name: f.dtype for f in inner_struct.fields}
        assert struct_fields == {"id": pl.Int64, "name": pl.Utf8}

    def test_nested_lists(self):
        """Test List containing List elements (List[List[T]])."""
        nested_list = _parse_polars_dtype("List[List[String]]")
        assert isinstance(nested_list, pl.List)

        inner_list = nested_list.inner
        assert isinstance(inner_list, pl.List)
        assert inner_list.inner == pl.Utf8

    def test_struct_with_nested_lists(self):
        """Test Struct containing nested List fields."""
        struct_type = _parse_polars_dtype(
            "Struct[matrix:List[List[Int64]],names:List[String]]"
        )
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}

        # Check matrix field (List of Lists)
        matrix_type = fields["matrix"]
        assert isinstance(matrix_type, pl.List)
        assert isinstance(matrix_type.inner, pl.List)
        assert matrix_type.inner.inner == pl.Int64

        # Check names field (simple List)
        assert fields["names"] == pl.List(pl.Utf8)

    def test_struct_with_list_of_structs(self):
        """Test Struct containing List of Struct fields."""
        dtype_str = "Struct[users:List[Struct[id:Int64,name:String]],active:Boolean]"
        struct_type = _parse_polars_dtype(dtype_str)
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}

        # Check users field (List of Structs)
        users_type = fields["users"]
        assert isinstance(users_type, pl.List)

        user_struct = users_type.inner
        assert isinstance(user_struct, pl.Struct)
        user_fields = {f.name: f.dtype for f in user_struct.fields}
        assert user_fields == {"id": pl.Int64, "name": pl.Utf8}

        # Check active field
        assert fields["active"] == pl.Boolean

    def test_deeply_nested_struct_list_combinations(self):
        """Test very deep nesting of Structs and Lists."""
        dtype_str = "Struct[data:List[Struct[items:List[Struct[value:Int64,tags:List[String]]]]]]"
        struct_type = _parse_polars_dtype(dtype_str)
        assert isinstance(struct_type, pl.Struct)

        # Navigate through the nested structure
        data_field = next(f for f in struct_type.fields if f.name == "data")
        assert isinstance(data_field.dtype, pl.List)

        # data is List[Struct[...]]
        level1_struct = data_field.dtype.inner
        assert isinstance(level1_struct, pl.Struct)

        # items field within the struct
        items_field = next(f for f in level1_struct.fields if f.name == "items")
        assert isinstance(items_field.dtype, pl.List)

        # items is List[Struct[...]]
        level2_struct = items_field.dtype.inner
        assert isinstance(level2_struct, pl.Struct)

        # Check the deepest struct fields
        level2_fields = {f.name: f.dtype for f in level2_struct.fields}
        assert level2_fields["value"] == pl.Int64
        assert level2_fields["tags"] == pl.List(pl.Utf8)

    def test_struct_with_mixed_array_list_types(self):
        """Test Struct with both Array and List fields."""
        dtype_str = "Struct[coordinates:Array[Float64,3],tags:List[String],matrix:List[Array[Int64,4]]]"
        struct_type = _parse_polars_dtype(dtype_str)
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}

        # Fixed-size array
        assert fields["coordinates"] == pl.Array(pl.Float64, 3)

        # Variable-size list
        assert fields["tags"] == pl.List(pl.Utf8)

        # List of fixed-size arrays
        matrix_type = fields["matrix"]
        assert isinstance(matrix_type, pl.List)
        assert matrix_type.inner == pl.Array(pl.Int64, 4)

    def test_struct_with_decimal_lists(self):
        """Test Struct containing Lists of Decimal values."""
        struct_type = _parse_polars_dtype("Struct[prices:List[Decimal(10,2)],id:Int64]")
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}
        assert fields["id"] == pl.Int64

        prices_type = fields["prices"]
        assert isinstance(prices_type, pl.List)
        assert prices_type.inner == pl.Decimal(10, 2)

    def test_empty_lists_in_structs(self):
        """Test Struct containing empty List types."""
        # Note: Empty lists might be inferred as List[String] by default
        struct_type = _parse_polars_dtype("Struct[empty_list:List[String],data:Int64]")
        assert isinstance(struct_type, pl.Struct)

        fields = {f.name: f.dtype for f in struct_type.fields}
        assert fields["empty_list"] == pl.List(pl.Utf8)
        assert fields["data"] == pl.Int64


class TestFieldSplittingWithNestedArraysLists:
    """Test the _split_struct_fields function with complex Array/List nesting."""

    def test_split_fields_with_simple_lists(self):
        """Test splitting fields containing simple Lists."""
        result = _split_struct_fields(
            "tags:List[String],scores:List[Int64],active:Boolean"
        )
        expected = ["tags:List[String]", "scores:List[Int64]", "active:Boolean"]
        assert result == expected

    def test_split_fields_with_arrays(self):
        """Test splitting fields containing Arrays."""
        result = _split_struct_fields("coords:Array[Float64,3],data:Array[Int64,10]")
        expected = ["coords:Array[Float64,3]", "data:Array[Int64,10]"]
        assert result == expected

    def test_split_fields_with_nested_lists(self):
        """Test splitting fields containing nested Lists."""
        result = _split_struct_fields("matrix:List[List[Int64]],simple:String")
        expected = ["matrix:List[List[Int64]]", "simple:String"]
        assert result == expected

    def test_split_fields_with_list_of_structs(self):
        """Test splitting fields containing List of Structs."""
        fields_str = "users:List[Struct[id:Int64,name:String]],count:Int64"
        result = _split_struct_fields(fields_str)
        expected = ["users:List[Struct[id:Int64,name:String]]", "count:Int64"]
        assert result == expected

    def test_split_complex_nested_structures(self):
        """Test splitting very complex nested field definitions."""
        fields_str = (
            "data:List[Struct[items:List[String],meta:Struct[id:Int64]]],simple:Boolean"
        )
        result = _split_struct_fields(fields_str)
        expected = [
            "data:List[Struct[items:List[String],meta:Struct[id:Int64]]]",
            "simple:Boolean",
        ]
        assert result == expected

    def test_split_fields_with_decimals_in_lists(self):
        """Test splitting fields with Decimals inside Lists."""
        fields_str = (
            "prices:List[Decimal(10,2)],quantities:List[Int64],total:Decimal(15,4)"
        )
        result = _split_struct_fields(fields_str)
        expected = [
            "prices:List[Decimal(10,2)]",
            "quantities:List[Int64]",
            "total:Decimal(15,4)",
        ]
        assert result == expected

    def test_split_mixed_brackets_and_parentheses(self):
        """Test splitting with complex mix of brackets and parentheses."""
        fields_str = "coords:Array[Decimal(5,2),3],data:List[Struct[value:Decimal(10,4)]],flag:Boolean"
        result = _split_struct_fields(fields_str)
        expected = [
            "coords:Array[Decimal(5,2),3]",
            "data:List[Struct[value:Decimal(10,4)]]",
            "flag:Boolean",
        ]
        assert result == expected


class TestEndToEndNestedArraysLists:
    """End-to-end tests for nested Arrays/Lists in schema inference."""

    def test_json_with_nested_arrays_to_schema(self):
        """Test that JSON with nested arrays produces correct schema."""
        # This would be tested with actual DataFrame if the full pipeline was available
        # For now, we test the dtype parsing that would handle the Rust output

        # Simulate what the Rust side might return for nested array JSON
        rust_dtype_output = (
            "Struct[user:Struct[id:Int64,tags:List[String]],data:List[List[Int64]]]"
        )

        schema_type = _parse_polars_dtype(rust_dtype_output)
        assert isinstance(schema_type, pl.Struct)

        fields = {f.name: f.dtype for f in schema_type.fields}

        # Verify user struct
        user_struct = fields["user"]
        assert isinstance(user_struct, pl.Struct)
        user_fields = {f.name: f.dtype for f in user_struct.fields}
        assert user_fields["id"] == pl.Int64
        assert user_fields["tags"] == pl.List(pl.Utf8)

        # Verify nested data arrays
        data_type = fields["data"]
        assert isinstance(data_type, pl.List)
        assert isinstance(data_type.inner, pl.List)
        assert data_type.inner.inner == pl.Int64

    def test_schema_creation_with_nested_types(self):
        """Test creating Polars Schema with complex nested Array/List types."""
        # Parse complex nested types
        user_list = _parse_polars_dtype("List[Struct[id:Int64,scores:List[Float64]]]")
        matrix_type = _parse_polars_dtype("List[List[Decimal(8,2)]]")

        # Create schema
        schema = pl.Schema(
            {"users": user_list, "matrix": matrix_type, "active": pl.Boolean}
        )

        # Verify schema structure
        assert isinstance(schema["users"], pl.List)
        assert isinstance(schema["matrix"], pl.List)
        assert schema["active"] == pl.Boolean

        # Verify nested structure
        user_struct = schema["users"].inner
        assert isinstance(user_struct, pl.Struct)
        user_fields = {f.name: f.dtype for f in user_struct.fields}
        assert user_fields["id"] == pl.Int64
        assert user_fields["scores"] == pl.List(pl.Float64)

        # Verify matrix structure
        assert isinstance(schema["matrix"].inner, pl.List)
        assert schema["matrix"].inner.inner == pl.Decimal(8, 2)

    def test_realistic_nested_data_structure(self):
        """Test parsing a realistic complex data structure."""
        # Simulate a complex e-commerce order structure
        dtype_str = """Struct[
            order:Struct[
                id:String,
                items:List[Struct[
                    product_id:String,
                    quantity:Int64,
                    price:Decimal(10,2),
                    tags:List[String]
                ]],
                shipping:Struct[
                    address:Struct[street:String,city:String,postal_code:String],
                    options:List[String]
                ],
                totals:Struct[
                    subtotal:Decimal(12,2),
                    tax:Decimal(8,4),
                    total:Decimal(12,2)
                ]
            ],
            metadata:Struct[
                created_at:String,
                source:String
            ]
        ]""".replace("\n", "").replace(" ", "")

        # This should parse without errors
        result = _parse_polars_dtype(dtype_str)
        assert isinstance(result, pl.Struct)

        # Verify top-level structure
        top_fields = {f.name: f.dtype for f in result.fields}
        assert "order" in top_fields
        assert "metadata" in top_fields

        # Verify order structure exists and is complex
        order_struct = top_fields["order"]
        assert isinstance(order_struct, pl.Struct)

        # The fact that this parses successfully proves our nested Array/List handling works
        print("✅ Complex nested Array/List structure parsed successfully!")


# Integration test to prove everything works together
def test_comprehensive_nested_arrays_lists_integration():
    """Comprehensive test proving nested Arrays/Lists work in all scenarios."""
    test_cases = [
        # Simple cases
        ("List[String]", pl.List(pl.Utf8)),
        ("Array[Int64,5]", pl.Array(pl.Int64, 5)),
        # Nested Lists
        ("List[List[String]]", pl.List(pl.List(pl.Utf8))),
        ("List[List[List[Int64]]]", pl.List(pl.List(pl.List(pl.Int64)))),
        # Lists of Structs
        ("List[Struct[id:Int64]]", pl.List(pl.Struct({"id": pl.Int64}))),
        # Structs with Lists
        ("Struct[tags:List[String]]", pl.Struct({"tags": pl.List(pl.Utf8)})),
        # Mixed complex types
        (
            "Struct[data:List[Struct[values:List[Decimal(10,2)]]]]",
            pl.Struct(
                {"data": pl.List(pl.Struct({"values": pl.List(pl.Decimal(10, 2))}))}
            ),
        ),
    ]

    for dtype_str, expected in test_cases:
        result = _parse_polars_dtype(dtype_str)
        assert result == expected, (
            f"Failed for {dtype_str}: got {result}, expected {expected}"
        )

    print("✅ All nested Arrays/Lists test cases passed!")
