# Options

Every optional parameter across the main functions, grouped by what it affects. The defaults, the functions that accept each option, and the descriptions are read from polars-genson itself when these docs are built, so they match the released package.

Function key: **ij** `infer_json_schema`, **ip** `infer_polars_schema`, **nj** `normalise_json` (the `df.genson` methods), **ifp** `infer_from_parquet`, **nfp** `normalise_from_parquet`.

## Input framing

| Option               | Default | Functions            | Description                                                      |
| -------------------- | ------- | -------------------- | ---------------------------------------------------------------- |
| `ndjson`             | `False` | ij, ip, nj, ifp, nfp | Whether to treat input as newline-delimited JSON                 |
| `ignore_outer_array` | `True`  | ij, ip, nj, ifp, nfp | Whether to treat top-level arrays as streams of objects          |
| `wrap_root`          | `None`  | ij, ip, nj, ifp, nfp | If a string, wrap each JSON row under that key before inference. |

## Maps and records

See [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/concepts/maps-and-records.md).

| Option                     | Default | Functions            | Description                                                                                      |
| -------------------------- | ------- | -------------------- | ------------------------------------------------------------------------------------------------ |
| `map_threshold`            | `20`    | ij, ip, nj, ifp, nfp | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden). |
| `map_max_required_keys`    | `None`  | ij, ip, nj, ifp, nfp | Maximum number of required keys allowed for Map inference.                                       |
| `force_field_types`        | `None`  | ij, ip, nj, ifp, nfp | Explicit overrides for specific fields.                                                          |
| `force_parent_field_types` | `None`  | ij, ip, nj, ifp, nfp | Explicit overrides for fields based on their parent field name.                                  |
| `unify_maps`               | `False` | ij, ip, nj, ifp, nfp | Enable unification of compatible but non-homogeneous record schemas into maps.                   |
| `no_unify`                 | `None`  | ij, ip, nj, ifp, nfp | Prevent unification of keys under these field names with their sibling record fields.            |
| `no_root_map`              | `True`  | ij, ip, nj, ifp, nfp | Prevent document root from becoming a map type, even if it meets map inference criteria          |

## Mixed types

See [Mixed types](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/concepts/mixed-types.md).

| Option                   | Default | Functions            | Description                                                                                                                                                                                               |
| ------------------------ | ------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `wrap_scalars`           | `True`  | ij, ip, nj, ifp, nfp | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). |
| `force_scalar_promotion` | `None`  | ij, ip, nj, ifp, nfp | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars.                                                                                            |
| `coerce_strings`         | `False` | ip, nj, nfp          | Infer the dtypes for rows that `normalise_json(coerce_strings=True)` normalises: strings beside numbers or booleans are to be coerced to them, so they are not given fields of their own.                 |

## Normalised output

See [Normalise a JSON column](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/guides/normalise.md).

| Option          | Default                                        | Functions   | Description                                                                                                                                                                                      |
| --------------- | ---------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `empty_as_null` | `True`                                         | nj, nfp     | If True, normalise empty arrays and empty maps to `null`.                                                                                                                                        |
| `map_encoding`  | `'mapping'` (ip) `None` (nj) `'mapping'` (nfp) | ip, nj, nfp | The `map_encoding` the JSON is normalised with: each map gets the dtype that encoding decodes to, a `pl.Map` for "mapping" and a list of `{key, value}` structs for "kv".                        |
| `decode`        | `True`                                         | nj          | Controls how the normalised JSON strings are decoded after normalisation:                                                                                                                        |
| `unnest`        | `True` (nj) `False` (nfp)                      | nj, nfp     | Whether to expand the decoded struct into a column per field, in place of a single column of structs.                                                                                            |
| `avro_schema`   | `None`                                         | nj, nfp     | An Avro schema (as JSON text or a dict) to normalise against in place of inferring one, for data whose schema is known: the inference pass is skipped, and the inference options have no effect. |

## Schema output

See [Infer a schema](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/guides/infer-schema.md).

| Option          | Default                                | Functions   | Description                                                                        |
| --------------- | -------------------------------------- | ----------- | ---------------------------------------------------------------------------------- |
| `avro`          | `False` (ij) `True` (ip) `False` (ifp) | ij, ip, ifp | Whether to output an Avro schema instead of a JSON Schema.                         |
| `merge_schemas` | `True`                                 | ij, ip      | Whether to merge schemas from all rows (True) or return individual schemas (False) |
| `schema_uri`    | `'http://json-schema.org/schema#'`     | ij, ifp     | Schema URI to use for the generated schema                                         |

## Parquet output

See [Normalise JSON stored in Parquet](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/guides/parquet.md).

| Option               | Default        | Functions | Description                                                                                                                                                                                                                  |
| -------------------- | -------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `output_path`        | `None`         | ifp       | Path to write schema JSON.                                                                                                                                                                                                   |
| `output_format`      | `'parquet'`    | nfp       | "arrow" writes no output file and returns the output's record batches as an Arrow C stream (the Arrow PyCapsule interface, `__arrow_c_stream__`), read once with `pyarrow.RecordBatchReader.from_stream` or `pyarrow.table`. |
| `output_column`      | `None`         | nfp       | Name for output column.                                                                                                                                                                                                      |
| `typed`              | `False`        | nfp       | If True, write the normalised rows as a typed struct column (the dtype `avro_to_polars_schema` gives for the inferred schema) instead of JSON strings, so no `str.json_decode` is needed after reading it back.              |
| `keep_columns`       | `None`         | nfp       | Input columns (e.g. an `id`) to copy unchanged into the output file, before the normalised column.                                                                                                                           |
| `schema`             | `None`         | nfp       | A target schema for the output column (the fields of its struct), for output that has the same schema whatever the rows held, such as chunks of one dataset normalised one at a time.                                        |
| `schema_check`       | `'compatible'` | nfp       | How the inferred schema is checked against `schema` before anything is written (see `check_schema_compatibility`).                                                                                                           |
| `extract_invariants` | `None`         | nfp       | Fields that are invariant per key, each mapped to its determinant: the sibling field whose value determines it, e.g. `{"labels": "id"}` when every object with the same `id` carries the same `labels`.                      |
| `lookup_output_path` | `None`         | nfp       | Parquet file for the lookup table, with string columns `field`, `key` and `value` (the subtree as JSON), in first-seen order.                                                                                                |
| `prune`              | `None`         | nfp       | Record fields, at any depth, whose records are pruned: wherever a record holds a non-null value for one, the record is removed, and each removed value is written to `prune_output_path`.                                    |
| `prune_output_path`  | `None`         | nfp       | Parquet file for the pruned values, one row each: the `keep_columns` of its row, `path` (the keys and indices from the row's root, as a JSON array) and `value` (the value as it was in the input, as JSON).                 |

## Performance and diagnostics

See [Process large inputs](https://lmmx.github.io/polars-genson/pr-preview/pr-320/reference/guides/large-inputs.md).

| Option         | Default    | Functions            | Description                                                      |
| -------------- | ---------- | -------------------- | ---------------------------------------------------------------- |
| `max_builders` | `None`     | ij, ip, nj, ifp, nfp | Maximum number of schema builders to create in parallel at once. |
| `profile`      | `False`    | ij, ip, nj, ifp, nfp | Whether to print profiling information                           |
| `debug`        | `False`    | ij, ip, ifp, nfp     | Whether to print debug information                               |
| `verbosity`    | `'Normal'` | ij, ip, ifp          | Whether to print verbose debug information                       |
