# 2026-10-06: Polars 2 Map dtype

## Current State
- Polars 2.0.0 loads the extension built against Rust polars 0.55.2 unmodified — 245 of 246 pre-existing Python tests passed on it before any change, the failure being `Categorical(ordering=...)` in the test file (schema_ser_deser_test.py:505)
- The Rust polars 0.55.2 crates have no `Map` dtype (only an Arrow `Map` array type) — `pl.Map` is constructed on the Python side only, and no `Map` dtype crosses the plugin boundary
- `avro_type_to_polars_type` and `json_type_to_polars_type` emit the dtype string `Map[String,V]` for Avro maps and for JSON Schema `additionalProperties` objects (polars-jsonschema-bridge/src/deserialise.rs:111, 168-170)
- `_parse_polars_dtype` turns `Map[String,V]` into `pl.Map` when `pl.Map` exists, else `List(Struct{key, value})` (dtypes.py:61-71)
- `map_encoding` defaults to `None` in `normalise_json`, `normalise_from_parquet` and `GensonNamespace.normalise_json`, resolving through `default_map_encoding()` to `"mapping"` on Polars 2+ and `"kv"` on Polars 1.x (dtypes.py:160, __init__.py:467, 777, 1274)
- `GensonNamespace.normalise_json(decode=...)` normalises with `"mapping"` when the target dtype contains a `pl.Map`, and with `"kv"` otherwise (__init__.py:1262-1270)
- `map_encoding="kv"` on Polars 2+ rewrites the inferred `pl.Map` dtypes to `List(Struct{key, value})` through `maps_to_entries` before `str.json_decode` (dtypes.py:169, __init__.py:1266)
- `normalise_from_parquet(typed=True)` accepts `map_encoding` of `"kv"` or `"mapping"`; `"mapping"` writes an Arrow `Map` column, stored as a Parquet map (parquet_io.rs:195, genson-core/src/parquet.rs:350-385)
- Polars 1.44.2 passes 249 tests with 8 skipped (the `pl.Map`-only tests in map_dtype_test.py); Polars 2.0.0 passes 253 with 4 skipped
- Tests asserting the entries shape pass `map_encoding="kv"` explicitly (decode_test.py, normalise_test.py, unify_maps_test.py, prune_test.py)

## Missing
- No Polars 2.0 entry in the CI matrix, and `polars>=1.43.2` stays the declared lower bound (polars-genson-py/pyproject.toml:30)
- `schema_to_json` serialises a `pl.Map` column as `List` of `Struct{key, value}`, so a `pl.Map` schema does not round-trip through `schema_to_json` and `json_to_schema` as a `pl.Map`
- `_dtype_to_dict` has no `pl.Map` branch, so `schema_to_dict` renders a map as the string `Map(String, Int64)` (__init__.py:1308-1316)
- genson-cli keeps `kv` as the `--map-encoding` default — the Polars version does not apply to the CLI

## Divergence
- The `normalise_json` docstring "Map encoding" list names `"kv"` as an option without noting that the JSON-string output of `decode=False` changes shape with the Polars-version default (`{"k": v}` on Polars 2+, `[{"key": ..., "value": ...}]` on 1.x)
