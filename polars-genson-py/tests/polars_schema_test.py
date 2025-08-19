import polars as pl
from polars_genson import infer_polars_schema


def test_basic_schema_inference():
    """Test basic JSON schema inference"""
    print("=== Test 1: Basic JSON Schema Inference ===")

    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "name": "Alice", "age": 30}',
                '{"id": 2, "name": "Bob", "age": 25}',
                '{"id": 3, "name": "Charlie", "age": 35}',
            ]
        }
    )

    print("Input data:")
    print(df)
    print()

    try:
        result = df.select(
            infer_polars_schema(
                pl.col("json_col"),
                ignore_outer_array=True,
                ndjson=False,
                merge_schemas=True,
                debug=True,
            )
        )

        print("Raw result:")
        print(result)
        print(f"Result type: {result.dtypes}")
        print()

        schema_fields = result.to_series().first()
        print("Schema fields:")
        for field in schema_fields:
            print(f"  {field['name']}: {field['dtype']}")
        print()

    except Exception as e:
        print(f"Error: {e}")
        return False

    return True


def test_mixed_types():
    """Test with mixed JSON types"""
    print("=== Test 2: Mixed Types ===")

    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "name": "Alice", "score": 95.5, "active": true}',
                '{"id": 2, "name": "Bob", "score": 87.2, "active": false}',
                '{"id": 3, "name": "Charlie", "score": 92.1, "active": true}',
            ]
        }
    )

    print("Input data:")
    print(df)
    print()

    try:
        result = df.select(
            infer_polars_schema(
                pl.col("json_col"),
                debug=False,
            )
        )

        schema_fields = result.to_series().first()
        print("Inferred schema:")
        for field in schema_fields:
            print(f"  {field['name']}: {field['dtype']}")
        print()

    except Exception as e:
        print(f"Error: {e}")
        return False

    return True


def test_nested_objects():
    """Test with nested objects"""
    print("=== Test 3: Nested Objects ===")

    df = pl.DataFrame(
        {
            "json_col": [
                '{"user": {"id": 1, "name": "Alice"}, "metadata": {"created": "2023-01-01"}}',
                '{"user": {"id": 2, "name": "Bob"}, "metadata": {"created": "2023-01-02"}}',
            ]
        }
    )

    print("Input data:")
    print(df)
    print()

    try:
        result = df.select(
            infer_polars_schema(
                pl.col("json_col"),
                debug=False,
            )
        )

        schema_fields = result.to_series().first()
        print("Inferred schema:")
        for field in schema_fields:
            print(f"  {field['name']}: {field['dtype']}")
        print()

    except Exception as e:
        print(f"Error: {e}")
        return False

    return True


def test_arrays():
    """Test with arrays"""
    print("=== Test 4: Arrays ===")

    df = pl.DataFrame(
        {
            "json_col": [
                '{"id": 1, "tags": ["python", "rust"], "scores": [1, 2, 3]}',
                '{"id": 2, "tags": ["javascript"], "scores": [4, 5]}',
            ]
        }
    )

    print("Input data:")
    print(df)
    print()

    try:
        result = df.select(
            infer_polars_schema(
                pl.col("json_col"),
                debug=False,
            )
        )

        schema_fields = result.to_series().first()
        print("Inferred schema:")
        for field in schema_fields:
            print(f"  {field['name']}: {field['dtype']}")
        print()

    except Exception as e:
        print(f"Error: {e}")
        return False

    return True


def main():
    print("Testing Polars Schema Inference\n")

    tests = [
        test_basic_schema_inference,
        test_mixed_types,
        test_nested_objects,
        test_arrays,
    ]

    passed = 0
    total = len(tests)

    for test in tests:
        try:
            if test():
                passed += 1
                print("✅ PASSED\n")
            else:
                print("❌ FAILED\n")
        except Exception as e:
            print(f"❌ FAILED with exception: {e}\n")

    print(f"Results: {passed}/{total} tests passed")

    if passed == total:
        print("🎉 All tests passed!")
    else:
        print("❌ Some tests failed")


if __name__ == "__main__":
    main()
