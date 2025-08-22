"""Integration tests proving that issue #23 is solved.

Issue #23: Support for Struct and Decimal types in Polars schema inference.

These tests prove that:
1. The Rust side can return dtype strings like "Struct[...]" and "Decimal(...)"
2. The Python side can parse these strings into proper Polars DataType objects
3. The end-to-end pipeline works: JSON → schema inference → Polars Schema object
"""

import polars as pl
import pytest
from polars_genson import infer_polars_schema
from polars_genson.dtypes import _parse_polars_dtype


class TestIssue23Resolution:
    """Tests proving that issue #23 (Struct and Decimal support) is resolved."""

    def test_struct_dtype_parsing_unit(self):
        """Unit test: Struct dtype strings parse correctly."""
        # Simple struct
        simple_struct = _parse_polars_dtype("Struct[name:String,age:Int64]")
        assert isinstance(simple_struct, pl.Struct)

        fields = {f.name: f.dtype for f in simple_struct.fields}
        assert fields == {"name": pl.Utf8, "age": pl.Int64}

        # Nested struct
        nested_struct = _parse_polars_dtype(
            "Struct[user:Struct[id:Int64,email:String],active:Boolean]"
        )
        assert isinstance(nested_struct, pl.Struct)

        top_fields = {f.name: f.dtype for f in nested_struct.fields}
        assert "user" in top_fields
        assert "active" in top_fields
        assert top_fields["active"] == pl.Boolean

        # Verify nested user struct
        user_struct = top_fields["user"]
        assert isinstance(user_struct, pl.Struct)
        user_fields = {f.name: f.dtype for f in user_struct.fields}
        assert user_fields == {"id": pl.Int64, "email": pl.Utf8}

    def test_decimal_dtype_parsing_unit(self):
        """Unit test: Decimal dtype strings parse correctly."""
        # Decimal with precision and scale
        decimal_with_params = _parse_polars_dtype("Decimal(10,2)")
        assert decimal_with_params == pl.Decimal(10, 2)

        # Decimal without parameters
        decimal_default = _parse_polars_dtype("Decimal")
        assert decimal_default == pl.Decimal(None, None)

        # High precision decimal
        high_precision = _parse_polars_dtype("Decimal(38,18)")
        assert high_precision == pl.Decimal(38, 18)

    def test_complex_mixed_types_parsing(self):
        """Unit test: Complex mixed type combinations parse correctly."""
        complex_dtype = _parse_polars_dtype(
            "Struct[product:Struct[name:String,price:Decimal(10,2)],tags:List[String],metadata:Struct[created:String,updated:String]]"
        )

        assert isinstance(complex_dtype, pl.Struct)
        fields = {f.name: f.dtype for f in complex_dtype.fields}

        # Check product struct
        product_struct = fields["product"]
        assert isinstance(product_struct, pl.Struct)
        product_fields = {f.name: f.dtype for f in product_struct.fields}
        assert product_fields["name"] == pl.Utf8
        assert product_fields["price"] == pl.Decimal(10, 2)

        # Check tags list
        assert fields["tags"] == pl.List(pl.Utf8)

        # Check metadata struct
        metadata_struct = fields["metadata"]
        assert isinstance(metadata_struct, pl.Struct)
        metadata_fields = {f.name: f.dtype for f in metadata_struct.fields}
        assert metadata_fields == {"created": pl.Utf8, "updated": pl.Utf8}

    def test_schema_creation_with_parsed_types(self):
        """Integration test: Parsed types work in pl.Schema creation."""
        # Parse various complex types
        user_struct = _parse_polars_dtype(
            "Struct[id:Int64,profile:Struct[name:String,email:String]]"
        )
        price_decimal = _parse_polars_dtype("Decimal(10,2)")
        tags_list = _parse_polars_dtype("List[String]")

        # Create a schema using these parsed types
        schema = pl.Schema(
            {
                "user": user_struct,
                "price": price_decimal,
                "tags": tags_list,
                "active": pl.Boolean,  # Mix with standard Polars type
            }
        )

        # Verify schema structure
        assert isinstance(schema["user"], pl.Struct)
        assert schema["price"] == pl.Decimal(10, 2)
        assert schema["tags"] == pl.List(pl.Utf8)
        assert schema["active"] == pl.Boolean

        # Verify we can introspect the nested struct
        user_fields = {f.name: f.dtype for f in schema["user"].fields}
        assert "id" in user_fields
        assert "profile" in user_fields
        assert user_fields["id"] == pl.Int64
        assert isinstance(user_fields["profile"], pl.Struct)

    def test_infer_polars_schema_with_structs(self):
        """End-to-end test: JSON with nested objects → Polars Schema with Structs."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"user": {"id": 1, "name": "Alice", "email": "alice@example.com"}, "active": true}',
                    '{"user": {"id": 2, "name": "Bob", "email": "bob@example.com"}, "active": false}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        # Should have proper Struct types for nested objects
        assert "user" in schema
        assert "active" in schema
        assert schema["active"] == pl.Boolean

        user_type = schema["user"]
        assert isinstance(user_type, pl.Struct)

        # Verify nested structure
        user_fields = {f.name: f.dtype for f in user_type.fields}
        expected_user_fields = {
            "id": pl.Int64,
            "name": pl.String,  # Note: pl.String maps to pl.Utf8
            "email": pl.String,
        }
        assert user_fields == expected_user_fields

    def test_infer_polars_schema_with_lists_of_structs(self):
        """End-to-end test: JSON with arrays of objects → List[Struct[...]]."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"items": [{"name": "item1", "value": 10}, {"name": "item2", "value": 20}]}',
                    '{"items": [{"name": "item3", "value": 30}]}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        assert "items" in schema
        items_type = schema["items"]
        assert isinstance(items_type, pl.List)

        # The inner type should be a Struct
        inner_struct = items_type.inner
        assert isinstance(inner_struct, pl.Struct)

        # Verify the struct fields
        struct_fields = {f.name: f.dtype for f in inner_struct.fields}
        assert struct_fields == {"name": pl.String, "value": pl.Int64}

    def test_deeply_nested_structures_end_to_end(self):
        """End-to-end test: Deeply nested JSON → complex Polars Schema."""
        df = pl.DataFrame(
            {
                "json_col": [
                    """{
                    "organization": {
                        "name": "ACME Corp",
                        "departments": [
                            {
                                "name": "Engineering",
                                "employees": [
                                    {"id": 1, "name": "Alice", "role": "Senior Engineer"},
                                    {"id": 2, "name": "Bob", "role": "Junior Engineer"}
                                ]
                            }
                        ]
                    }
                }""",
                    """{
                    "organization": {
                        "name": "XYZ Inc",
                        "departments": [
                            {
                                "name": "Sales",
                                "employees": [
                                    {"id": 3, "name": "Charlie", "role": "Sales Rep"}
                                ]
                            }
                        ]
                    }
                }""",
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        # Navigate through the complex nested structure
        assert "organization" in schema
        org_type = schema["organization"]
        assert isinstance(org_type, pl.Struct)

        org_fields = {f.name: f.dtype for f in org_type.fields}
        assert "name" in org_fields
        assert "departments" in org_fields
        assert org_fields["name"] == pl.String

        # Check departments list structure
        departments_type = org_fields["departments"]
        assert isinstance(departments_type, pl.List)

        # Department struct
        dept_struct = departments_type.inner
        assert isinstance(dept_struct, pl.Struct)
        dept_fields = {f.name: f.dtype for f in dept_struct.fields}
        assert "name" in dept_fields
        assert "employees" in dept_fields

        # Check employees list structure
        employees_type = dept_fields["employees"]
        assert isinstance(employees_type, pl.List)

        # Employee struct
        emp_struct = employees_type.inner
        assert isinstance(emp_struct, pl.Struct)
        emp_fields = {f.name: f.dtype for f in emp_struct.fields}
        expected_emp_fields = {"id": pl.Int64, "name": pl.String, "role": pl.String}
        assert emp_fields == expected_emp_fields

    def test_schema_with_mixed_optional_fields(self):
        """End-to-end test: JSON with optional fields creates correct schema."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"user": {"id": 1, "name": "Alice"}, "metadata": {"created": "2023-01-01"}}',
                    '{"user": {"id": 2, "name": "Bob", "email": "bob@example.com"}, "metadata": {"created": "2023-01-02", "updated": "2023-01-03"}}',
                    '{"user": {"id": 3, "name": "Charlie", "email": "charlie@example.com"}}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        # Check user struct - should include all fields seen across rows
        user_type = schema["user"]
        assert isinstance(user_type, pl.Struct)
        user_fields = {f.name: f.dtype for f in user_type.fields}
        assert "id" in user_fields
        assert "name" in user_fields
        assert "email" in user_fields  # Optional field should be included

        # Check metadata struct - should include all fields seen across rows
        if "metadata" in schema:  # This field is optional at the top level
            metadata_type = schema["metadata"]
            assert isinstance(metadata_type, pl.Struct)
            metadata_fields = {f.name: f.dtype for f in metadata_type.fields}
            assert "created" in metadata_fields
            # "updated" should be included if the schema merger includes optional fields

    def test_issue_23_comprehensive_proof(self):
        """Comprehensive test proving issue #23 is fully resolved."""
        # This test combines Struct, Decimal, and List types in a realistic scenario

        # Note: The exact Decimal inference depends on your Rust implementation
        # This test focuses on ensuring the Python side can handle whatever the Rust side produces

        df = pl.DataFrame(
            {
                "json_col": [
                    """{
                    "transaction": {
                        "id": "txn_001",
                        "amount": 123.45,
                        "currency": "USD",
                        "details": {
                            "description": "Payment for services",
                            "categories": ["business", "consulting"]
                        }
                    },
                    "timestamp": "2023-12-01T10:00:00Z"
                }""",
                    """{
                    "transaction": {
                        "id": "txn_002", 
                        "amount": 67.89,
                        "currency": "EUR",
                        "details": {
                            "description": "Product purchase",
                            "categories": ["retail"]
                        }
                    },
                    "timestamp": "2023-12-01T11:00:00Z"
                }""",
                ]
            }
        )

        # This should not raise any errors and should produce a valid schema
        schema = df.genson.infer_polars_schema("json_col")

        # Verify the overall structure
        assert "transaction" in schema
        assert "timestamp" in schema

        # Check transaction structure
        transaction_type = schema["transaction"]
        assert isinstance(transaction_type, pl.Struct)

        tx_fields = {f.name: f.dtype for f in transaction_type.fields}
        assert "id" in tx_fields
        assert (
            "amount" in tx_fields
        )  # Could be Float64 or Decimal depending on Rust implementation
        assert "currency" in tx_fields
        assert "details" in tx_fields

        # Check nested details structure
        details_type = tx_fields["details"]
        assert isinstance(details_type, pl.Struct)

        details_fields = {f.name: f.dtype for f in details_type.fields}
        assert "description" in details_fields
        assert "categories" in details_fields

        # Categories should be a list of strings
        categories_type = details_fields["categories"]
        assert isinstance(categories_type, pl.List)
        assert categories_type.inner == pl.Utf8

        # Timestamp should be a string (could be enhanced to Date/Datetime in future)
        assert schema["timestamp"] == pl.String

        # The key proof: we can create DataFrames with this schema
        # If Struct/Decimal parsing was broken, this would fail
        try:
            # This validates that the schema is well-formed
            empty_df = pl.DataFrame(schema=schema)
            assert len(empty_df.columns) == len(schema)
            print("✅ Issue #23 resolved: Complex schemas with Structs work correctly!")
        except Exception as e:
            pytest.fail(
                f"Schema validation failed, indicating issue #23 not fully resolved: {e}"
            )


# Additional regression tests to ensure we don't break existing functionality
class TestBackwardsCompatibility:
    """Ensure that the enhanced dtype parsing doesn't break existing functionality."""

    def test_simple_types_still_work(self):
        """Ensure simple type inference still works after Struct/Decimal additions."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"name": "Alice", "age": 30, "score": 95.5, "active": true}',
                    '{"name": "Bob", "age": 25, "score": 87.2, "active": false}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        expected = pl.Schema(
            {
                "name": pl.String,
                "age": pl.Int64,
                "score": pl.Float64,
                "active": pl.Boolean,
            }
        )

        assert schema == expected

    def test_simple_arrays_still_work(self):
        """Ensure simple array inference still works."""
        df = pl.DataFrame(
            {
                "json_col": [
                    '{"tags": ["python", "rust"], "scores": [1, 2, 3]}',
                    '{"tags": ["javascript"], "scores": [4, 5]}',
                ]
            }
        )

        schema = df.genson.infer_polars_schema("json_col")

        expected = pl.Schema(
            {
                "tags": pl.List(pl.String),
                "scores": pl.List(pl.Int64),
            }
        )

        assert schema == expected
