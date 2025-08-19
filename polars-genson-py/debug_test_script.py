#!/usr/bin/env python3
"""Debug script to isolate the segfault."""

print("Starting debug script...")

try:
    print("1. Importing polars...")
    import polars as pl

    print("   ✓ Polars imported")

    print("2. Importing polars_genson...")
    import polars_genson

    print("   ✓ polars_genson imported")

    print("3. Creating DataFrame...")
    df = pl.DataFrame(
        {"json_data": ['{"name": "Alice", "age": 30}', '{"name": "Bob", "age": 25}']}
    )
    print("   ✓ DataFrame created")
    print(f"   DataFrame shape: {df.shape}")

    print("4. Checking if genson namespace exists...")
    if hasattr(df, "genson"):
        print("   ✓ genson namespace found")
    else:
        print("   ✗ genson namespace NOT found")
        exit(1)

    print("5. Calling infer_json_schema with debug...")
    try:
        schema = df.genson.infer_json_schema("json_data", debug=True)
        print("   ✓ Schema inference completed!")
        print(f"   Schema type: {type(schema)}")

    except Exception as e:
        print(f"   ✗ Schema inference failed: {e}")
        import traceback

        traceback.print_exc()

except Exception as e:
    print(f"Error during setup: {e}")
    import traceback

    traceback.print_exc()
