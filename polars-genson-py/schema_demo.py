"""Checking if we can access a DF."""

import polars as pl
import polars_genson

df = pl.DataFrame({"a": [1, 2, 3], "b": [True, False, True]})
schema = df.schema

jsonified_schema = polars_genson.schema_to_json(schema)
print("(🐍) Got some JSON:")
print(jsonified_schema)

print()

reschemafied = polars_genson.json_to_schema(jsonified_schema)
print("(🐍) Turned that JSON back into a schema:")
print(reschemafied)
