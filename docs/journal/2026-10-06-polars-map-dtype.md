# 2026-10-06: Polars 2 Map dtype

## Current State
- Polars 2.0.0 loads the extension built against Rust polars 0.55.2 unmodified — 245 of 246 pre-existing Python tests passed on it before any change, the failure being `Categorical(ordering=...)` in the test file (schema_ser_deser_test.py:505)
- The Rust polars 0.55.2 crates have no `Map` dtype (only an Arrow `Map` array type) — `pl.Map` is constructed on the Python side only, and no `Map` dtype crosses the plugin boundary
- `avro_type_to_polars_type` and `json_type_to_polars_type` emit the dtype string `Map[String,V]` for Avro maps and for JSON Schema `additionalProperties` objects (polars-jsonschema-bridge/src/deserialise.rs:111, 168-170)
- `_parse_polars_dtype` turns `Map[String,V]` into `pl.Map` when `pl.Map` exists, else `List(Struct{key, value})` (dtypes.py:61-71)
- `map_encoding` defaults to `None` in `normalise_json`, `normalise_from_parquet` and `GensonNamespace.normalise_json`, resolving through `default_map_encoding()` to `"mapping"` on Polars 2+ and `"kv"` on Polars 1.x (dtypes.py:160, __init__.py:467, 777, 1274)
- `GensonNamespace.normalise_json(decode=...)` normalises with `"mapping"` when the target dtype contains a `pl.Map`, and with `"kv"` otherwise (__init__.py:1262-1270)
- `map_encoding="kv"` on Polars 2+ rewrites the inferred `pl.Map` dtypes to `List(Struct{key, value})` through `maps_to_entries` before `str.json_decode` (dtypes.py:169, __init__.py:1266)
- `normalise_from_parquet(typed=True)` accepts `map_encoding` of `"kv"` or `"mapping"`; `"mapping"` writes an Arrow `Map` column, stored as a Parquet map (parquet_io.rs:195, genson-core/src/parquet.rs:373-390)
- Polars 1.44.2 passes 252 tests with 9 skipped (the `pl.Map`-only tests in map_dtype_test.py); Polars 2.0.0 passes 257 with 4 skipped
- Tests asserting the entries shape pass `map_encoding="kv"` explicitly (decode_test.py, normalise_test.py, unify_maps_test.py, prune_test.py)
- Polars 1.44.2 reads a Parquet map as `List(Struct{key, value})`, the `kv` dtype, so typed `"mapping"` output reads back as `avro_to_polars_schema`'s dtype on both versions (map_dtype_test.py `test_typed_parquet_maps`)
- `schema_to_dict` renders a `pl.Map` as `{"map": {"key": ..., "value": ...}}` (__init__.py `_dtype_to_dict`)
- `avro_to_arrow_type` and `avro_record_fields` take `native_map: bool` rather than `MapEncoding`: `parquet.rs` is behind the `parquet` feature and `MapEncoding` behind `avro`, and neither enables the other
- genson-core's `empty_objects.rs` writes the always-empty map as both `List<Struct>` and a native `Map<Utf8, Null>`

## Missing
- CI builds wheels but runs no Python tests, so neither Polars version is exercised there, and `polars>=1.43.2` stays the declared lower bound (polars-genson-py/pyproject.toml:30)
- `schema_to_json` serialises a `pl.Map` column as `List` of `Struct{key, value}`, so a `pl.Map` schema does not round-trip through `schema_to_json` and `json_to_schema` as a `pl.Map`
- genson-cli keeps `kv` as the `--map-encoding` default — the Polars version does not apply to the CLI
