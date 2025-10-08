# standalone_parquet_demo.py
"""Standalone demo for Parquet I/O functionality."""

import json

import polars as pl
from polars_genson import infer_from_parquet, normalise_from_parquet

input_file = "../tests/data/claims_fixture_x4.parquet"
output_file = "demo_output.parquet"
schema_file = "demo_schema.json"

print("=" * 60)
print("DEMO: Parquet I/O with polars-genson")
print("=" * 60)

# 1. Show what's in the input file
print("\n1. Input Parquet file:")
df_input = pl.read_parquet(input_file)
print(df_input)
print(f"\nColumn 'claims' contains JSON strings")
print(f"First row sample: {df_input['claims'][0][:100]}...")

# 2. Infer schema and save to file
print("\n2. Inferring schema from Parquet column...")
infer_from_parquet(
    input_file,
    column="claims",
    output_path=schema_file,
    map_threshold=0,
    unify_maps=True,
    wrap_root="claims",
)

print(f"\n✅ Schema written to: {schema_file}")
with open(schema_file) as f:
    schema = json.load(f)
    print(f"Schema type: {schema.get('type')}")
    print(f"Schema has {len(str(schema))} characters")

# 3. Normalize the data and write to new Parquet
print("\n3. Normalizing JSON data...")
normalise_from_parquet(
    input_file,
    column="claims",
    output_path=output_file,
    output_column="claims_normalized",
    map_threshold=0,
    unify_maps=True,
    wrap_root="claims",
)

print(f"\n✅ Normalized data written to: {output_file}")

# 4. Show the output
print("\n4. Output Parquet file:")
df_output = pl.read_parquet(output_file)
print(df_output)
print(f"\nFirst normalized row:")
print(json.dumps(json.loads(df_output["claims_normalized"][0]), indent=2))

print("\n" + "=" * 60)
print("DEMO COMPLETE!")
print("=" * 60)
print(f"\nGenerated files:")
print(f"  - {schema_file} (schema JSON)")
print(f"  - {output_file} (normalized Parquet)")
print("\nYou can inspect these files now!")
