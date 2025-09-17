"""Demo of unify_maps and debug features."""

import polars as pl
import polars_genson

# Create a DataFrame with JSON data that will trigger unification
json_data = [
    '{"claims": {"P31": [{"mainsnak": {"property": "P31"}, "rank": "normal"}], "P569": [{"rank": "normal"}]}}',
    '{"claims": {"P123": [{"mainsnak": {"property": "P123"}, "rank": "preferred"}], "P456": [{"rank": "deprecated"}]}}',
    '{"claims": {"P789": [{"mainsnak": {"property": "P789"}, "rank": "normal"}], "P101": [{"rank": "normal"}]}}',
]

df = pl.DataFrame({"json_col": json_data})

print("Demo JSON data:")
for i, data in enumerate(json_data):
    print(f"Row {i + 1}: {data}")
print()

# Infer schema with unify_maps=True and debug=True
print("Inferring schema with unify_maps=True and debug=True:")
print("=" * 60)

schema_expr = polars_genson.infer_json_schema(
    "json_col",
    unify_maps=True,
    debug=True,
    map_threshold=2,  # Lower threshold to trigger map detection
)

print("Inferred schema:")
print("=" * 60)
print(df.select(schema_expr).item())
