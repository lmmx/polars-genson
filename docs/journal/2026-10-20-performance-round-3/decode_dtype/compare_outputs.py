"""Output equality, parent against child: normalise_sitelinks' frame written by one run of
each. DataFrame.equals is left out: on List(Struct{.., List(String)}) Polars 2.0.0
takes 27 s for 50 rows. Usage: python -I compare_outputs.py <parent.parquet> <child.parquet>"""
import json
import sys

import polars as pl

a, b = pl.read_parquet(sys.argv[1]), pl.read_parquet(sys.argv[2])
print(json.dumps({"parent": sys.argv[1].rsplit("/", 1)[-1], "child": sys.argv[2].rsplit("/", 1)[-1],
                  "schema_equal": a.schema == b.schema, "ndjson_equal": a.write_ndjson() == b.write_ndjson(),
                  "dicts_equal": a.to_dicts() == b.to_dicts(), "rows": a.height, "schema": str(a.schema)}))
