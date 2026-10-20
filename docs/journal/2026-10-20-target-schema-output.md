# 2026-10-20: Typed Parquet output in a target schema

`normalise_from_parquet(typed=True, schema=..., schema_check=..., unnest=...)` writes
typed output in the shape of a stored schema, after checking the inferred schema against
it. wikidata-pq gets its chunk tables into that shape today by reading genson's file back
with Polars, conforming it and writing it again. This entry records the measurements that
led to the change, the compatibility contract, the API, how it is implemented, and how it
was validated on one official-dump chunk. Scripts are in
`2026-10-20-target-schema-output/scripts/` and their outputs in `logs/`.

## The bottleneck

The workload is chunk 0 of the official 20261012 Wikidata JSON dump, as wikidata-pq's
split-dump and route-release make it:

- the first 10,000 entities of the dump (Q31 to Q83751), streamed from the bz2 file and
  reshaped by `dump.write_chunk` (`scripts/first_chunk.py`, which also wrote a chunk 1,
  deleted unused), then routed by `dump.route_chunk` without scholarly works;
- 485 MB of JSON in a 48.8 MB Parquet file, 374.8 MB of it claims, 13.2 MB labels,
  12.6 MB descriptions, 4.3 MB aliases and 28.7 MB sitelinks.

wikidata-pq's `process()` at 5ad5f96, timed from outside by wrapping its functions
(`scripts/profile_process.py`), on polars-genson 1.1.0 and Polars 2.0.0 with
`map_encoding="kv"` passed to genson (`logs/new_c0.json`), took 44.6 s cold and 41.6 s
and 41.8 s warm. The warm runs split as:

| step | wall (s) | share |
|---|---|---|
| `sink_parquet` of the conformed claims | 31.8 | 76% |
| `scan_parquet(schema=claims_schema).unnest("claims").collect()` | 3.6 | 9% |
| genson `normalise_from_parquet` of the claims | 3.3 | 8% |
| sitelinks (`normalise_json`, then `sink_parquet`) | 1.2 | 3% |
| labels, descriptions, aliases (genson, `cast`, `sink_parquet`) | 1.1 | 3% |
| the rest (reading the chunk, id checks) | 0.7 | 2% |

- The claims sink peaked 1.16 GB above the process's resident memory before it, and the
  profiler's process peaked at 4.15 GB (three runs).
- The sink decoded and re-encoded a claims file genson had already encoded once (and
  wrote 57.6 MB), to add the fields the chunk lacked and to move the struct's fields to
  top-level columns.
- On master's lock (polars-genson 0.9.6, Polars 1.44.2, `logs/locked_master_c0.json`) the
  same chunk took 71.1 s cold, 38.3 s of it in genson's claims normalisation and 21.2 s in
  the claims sink.

## Why direct output in the target schema

The conformance Polars does is structural: struct fields in the stored schema's order,
fields the chunk lacked as nulls, fields typed `Null` (never seen with a value) as nulls
of the stored dtype, then `unnest`. genson holds the decoded rows as Arrow arrays just
before it writes them, so the same reshaping can be applied to those arrays (rebuilding
containers over the existing buffers) and the file written once. The checks that
`process()` relies on keep their inputs:

- the inferred schema stays in the file's `genson_avro_schema` metadata, so the
  schema-drift check reads it as before;
- the decision whether the inferred schema fits the stored one moves before the write, so
  an incompatible chunk writes nothing.

Writing the conformed frame with pyarrow instead of Polars was measured on an earlier
60-row chunk (1.2 s for Polars, 0.26 to 0.46 s for pyarrow, 0.056 s for genson's own
write): any second writer still decodes and re-encodes the file.

## The compatibility contract

`schema_check="compatible"` (the default when a target schema is given) accepts an
inferred schema when, recursively:

| inferred | target | verdict |
|---|---|---|
| `Null` | any dtype | fits: written as nulls of the target dtype |
| a scalar dtype | the same dtype | fits (32- and 64-bit offsets of strings and lists count as the same) |
| a scalar dtype | another scalar dtype | does not fit: no widening (`Int64` into `Float64` is rejected) |
| any non-null dtype | `Null` | does not fit: a value in a field the store has only seen null |
| a struct | a struct | fits when each of its fields is one of the target's and fits it; the target's other fields are written as nulls |
| a list, an array of size n, a map | the same kind (array of the same size) | fits when the items (key and value) fit |
| anything else | | does not fit (`r: String, the target has Struct`) |

The contract reproduces wikidata-pq's `is_acceptable_diff` (process.py), a DeepDiff of
`schema_to_dict` of the stored and inferred schemas, whose behaviour was probed case by
case:

- DeepDiff's `dictionary_item_removed` (a stored field the chunk lacks, at any depth)
  passes there and under "compatible";
- DeepDiff's `type_changes`/`values_changed` to `"Null"` (an inferred `Null` leaf, struct
  or list) passes there and under "compatible";
- `dictionary_item_added` (a field the stored schema lacks) fails in both;
- any other type change, including `Int64` to `Float64`, fails in both;
- a stored `pl.Null` field (`altitude`) with an inferred dtype fails in both.

Two differences are deliberate:

- `schema_to_dict` writes a list as `{"list": ...}` and a map as `{"map": ...}`, so a struct
  field named `list` or `map` is indistinguishable from a list or a map there; the new
  check compares dtypes, not their dict renderings.
- The new check names each offending path (`claims[].value[].mainsnak.n: a field the target
  schema lacks (Int64)`, with `[]` for list items and `{}` for map values) rather than a
  DeepDiff report.

`schema_check="strict"` also reports target fields the inferred schema lacks (`r.y:
missing, and the schema check is strict`) and does not let `Null` fit other dtypes.
`schema_check="none"` checks nothing: fields the target lacks are dropped, a `Null`
target takes nulls, and other scalar dtypes are cast with Arrow's `cast`.

On chunk 0 the claims' inferred schema differs from wikidata-pq's `claims_schema` by 13
`dictionary_item_removed` entries (stored fields absent from this chunk, among them
`mainsnak.hash`, `latitude__integer` and the references' snak `hash`), all accepted by
both checks (`scripts/metadata_schema.py`); the schema recovered from the file's metadata
with `map_encoding="kv"` equals the file's own schema in genson's untargeted output.

## API

- `normalise_from_parquet(..., typed=True, schema=None, schema_check="compatible",
  unnest=False)` (polars-genson-py/python/polars_genson/__init__.py:675):
  - `schema` takes a `pl.Schema` (the output column's struct fields) or a `pl.Struct`;
  - `unnest=True` writes the struct's fields as top-level columns after the
    `keep_columns`, a null row's fields as nulls, as Polars' `unnest` makes them, and is
    independent of `schema`;
  - `schema` or `unnest` without `typed=True` raises `ValueError("schema and unnest need
    typed output (typed=True)")` (polars-genson-py/src/parquet_io.rs:211);
  - an inferred schema that fails the check raises `ValueError` naming each path before
    the output, lookup or prune files are written (parquet_io.rs:424; the lookup write
    moved after the check, parquet_io.rs:431).
- `check_schema_compatibility(inferred, target, schema_check="compatible") -> list[str]`
  (__init__.py:885) gives the same verdicts as a list of problems, empty when the schema
  fits, for a caller that has an inferred schema without writing (wikidata-pq's stored
  schemas against a chunk's metadata).
- `dtype_spec` (polars-genson-py/python/polars_genson/dtypes.py:224) describes a Polars
  dtype as JSON for the Rust side: dtype names (`"String"`, `"Int64"`, `"Null"`, ...) and
  `{"Struct": [[name, dtype], ...]}`, `{"List": dtype}`, `{"Array": [dtype, size]}`,
  `{"Map": [key, value]}`.
- With no `schema` and `unnest=False`, `normalise_from_parquet` writes what it wrote
  before: the same Arrow schema and the same writer settings (the Python suite, 574
  tests with the new ones, passes with no existing test changed).

## Implementation

- `genson_core::conform` (genson-core/src/conform.rs, behind the `parquet` feature):
  - `SchemaCheck` (conform.rs:26) parses `"compatible"`, `"strict"` and `"none"`;
  - `incompatibilities` (conform.rs:104) walks the inferred and target Arrow dtypes
    together, one message per offending path;
  - `conform` (conform.rs:221) rebuilds an array in the target's shape: an equal dtype
    returns the same `Arc`, a `Null` array becomes `new_null_array` of the target dtype,
    a struct takes the target's fields in the target's order (missing ones as
    `new_null_array`) over the existing children and null buffer, lists, arrays and maps
    are rebuilt over conformed values with their offsets and nulls, and strings fall to
    Arrow's `cast`;
  - `unnest` (conform.rs:343) returns a struct's children with the struct's nulls unioned
    into each child's;
  - `data_type_from_spec` (conform.rs:374) turns `dtype_spec`'s JSON into the Arrow dtype
    Polars writes to Parquet for that dtype: `LargeUtf8` strings, `LargeBinary` binaries,
    `LargeList` lists, all fields nullable.
- `genson_core::parquet::write_struct_columns` (genson-core/src/parquet.rs:742) takes a
  `StructLayout` (parquet.rs:732): `unnest`, and `spec_names`, which sets arrow-rs's
  `coerce_types` so the Parquet schema names a list's groups `list` and `element` (and a
  map's `key_value`), as Polars and pyarrow write them, where arrow-rs writes the Arrow
  field name (`item`). `write_struct_column_with` calls it with the default layout.
- In `normalise_from_parquet` (parquet_io.rs), the target spec is parsed (it must be a
  struct), the inferred Arrow fields are checked against it after inference, the decoded
  batches are conformed in parallel (rayon, one batch per task, the `conform` profile
  tick), and the file is written with the target's fields, `unnest` and `spec_names`.
  The metadata keeps `genson_avro_schema` (the inferred, pruned schema) and
  `genson_normalise_config`.
- `schema_incompatibilities` (parquet_io.rs:557) exposes `incompatibilities` to Python
  for `check_schema_compatibility`.

## Surprises and trade-offs

- The first equivalence run found every table equal as Polars frames but not as files:
  genson wrote `list<item: ...>` and `string` where Polars writes `large_list<element:
  ...>` and `large_string`. Polars reads both into the same frame, but a reader that
  takes the Arrow schema from the file (pyarrow, Hugging Face `datasets`) sees two
  schemas across chunks written before and after the change. Target dtypes therefore use
  Polars' 64-bit offsets, and the writer the Parquet specification's list names, so the
  file's Arrow schema and Parquet columns (paths, physical and logical types, definition
  and repetition levels) equal those of the frame written by Polars.
- Widening 32-bit offsets to 64-bit copies each offsets buffer (the string values buffers
  are shared): the claims' `conform` tick went from 0.018 s before the change to 0.09 to
  0.12 s after it.
- `DataFrame.equals` on two claims frames ran for more than 8 minutes without finishing
  (a row-by-row kernel for nested lists, polars-compute `array_tot_eq_missing_kernel`, by
  gdb); the comparisons use pyarrow's `Table.equals` after `Schema.equals`, which takes
  seconds.
- Under `schema_check="none"`, conforming a list to a `Null` target first failed in Arrow's
  `cast` ("Cannot cast LIST to non-list data type Null"); a `Null` target now takes nulls
  of any array.
- genson writes zstd level 1, and Polars' `sink_parquet` its default zstd level (3): on
  chunk 0 genson's claims file is 54.3 MB against Polars' 57.6 MB, and its labels,
  descriptions and aliases 10 to 22% larger (4.2, 4.5 and 1.8 MB against 3.8, 3.7 and 1.6).
- The claims sink took 21.2 s on master's lock (Polars 1.44.2) and 31.8 s on Polars 2.0.0,
  over the outputs of different genson versions (0.9.6 and 1.1.0).

## Equivalence on chunk 0

`scripts/equiv_target.py` runs, for claims, labels, descriptions and aliases, both
routes three times, alternating, on polars-genson at this branch (release build) and
Polars 2.0.0 (`logs/equiv_target.json`, `logs/equiv_target.log`):

- today's route, wikidata-pq's own `normalise_claims_direct` and `normalise_map_direct`
  then `atomic_sink_parquet`;
- the direct route, `normalise_from_parquet(schema=..., unnest=True)` with the same
  options, the inferred schema read from the metadata and passed through both
  `is_acceptable_diff` and `check_schema_compatibility` (both accept), then a rename.

| table | rows | Polars schema | Arrow schema | rows (Arrow) |
|---|---|---|---|---|
| labels | 10,000 | equal | equal | equal |
| descriptions | 10,000 | equal | equal | equal |
| aliases | 10,000 | equal | equal | equal |
| claims | 10,000 | equal | equal | equal |
| claims lookup | 0 (an official dump's claims carry no labels) | equal | equal | equal |

wikidata-pq's whole `process()` with the direct route (its branch
feat/genson-target-schema) against master's `process()` on master's lock
(`scripts/compare_outputs.py`, `logs/compare_master_branch.json`): labels, descriptions,
aliases, entities, links and claims_labels are equal in rows, Polars schema, Arrow
schema and Parquet columns. The claims are equal in schemas and Parquet columns and
differ only in struct validity (`scripts/diff_claims.py`,
`logs/diff_claims_master_branch.txt`) — master writes a struct whose fields are all null
where the JSON has no such object, and the branch writes a null struct:

| path under `claims[].value[]` | rows |
|---|---|
| `mainsnak.datavalue` (somevalue and novalue snaks, 100 and 205) | 305 |
| `mainsnak.datavalue.precision` | 603,420 |
| `mainsnak.datavalue.latitude` and `.longitude` | 613,258 each |
| `references[].snaks[].value[].datavalue` | 31 |

Every leaf value under those rows is null on both sides. The direct route's claims equal
those of wikidata-pq's unchanged code on genson 1.1.0 and Polars 2.0.0 (the table above),
so the validity difference comes with the dependency upgrade, not with the target schema.

## Before and after

Per table, warm rounds 2 and 3 of `scripts/equiv_target.py` (wall s):

| table | today's route | direct |
|---|---|---|
| claims | 30.80, 30.06 | 2.95, 2.99 |
| labels | 0.40, 0.42 | 0.16, 0.22 |
| descriptions | 0.33, 0.30 | 0.17, 0.18 |
| aliases | 0.22, 0.21 | 0.08, 0.07 |

The claims' `normalise_from_parquet` in the direct route (warm, from its profile ticks):
inference 0.60 to 0.69 s, parse, normalise and decode 0.53 to 0.55 s, `conform` 0.10 to
0.12 s, the write 1.11 to 1.21 s (1.08 and 0.96 s for the untargeted write).

The claims step alone in a fresh process (`scripts/mem_claims.py`, `logs/mem_claims.txt`):
32.2 s and 3,523 MB peak resident memory above a 145 MB base today, 4.1 s and 1,893 MB
direct.

wikidata-pq's `process()` on chunk 0 (`scripts/profile_process.py`):

| | polars-genson, Polars | runs (wall s) | CPU (s) | peak RSS of the profiler |
|---|---|---|---|---|
| master's lock | 0.9.6, 1.44.2 | 71.1 (one cold run) | 148.3 | 4.52 GB |
| master's code, upgraded | 1.1.0, 2.0.0 | 44.6, 41.6, 41.8 | 59.6, 52.0, 51.9 | 4.15 GB |
| feat/genson-target-schema | this branch, 2.0.0 | 6.5, 5.4, 5.3 | 18.8, 15.1, 15.0 | 3.37 GB |

In the branch's warm runs the claims take 3.3 s of 5.3 s; the remainder is reading the
chunk (0.5 s), sitelinks (0.7 s genson, 0.3 s sink) and the maps (0.4 s).

## Current State

- Typed output takes a target schema, checked under "compatible", "strict" or "none" and
  conformed by `genson_core::conform` (conform.rs), with or without `unnest`
  (parquet.rs:732-742).
- A file written to a target schema has the Arrow schema and Parquet columns Polars
  writes for the same frame, with `genson_avro_schema` holding the inferred schema
  (polars-genson-py/tests/target_schema_test.py).
- `check_schema_compatibility` reproduces wikidata-pq's `is_acceptable_diff` on its cases
  (target_schema_test.py, genson-core/src/tests/conform.rs).

## Missing

- Target dtypes beyond strings, binaries, booleans, nulls, integers, floats, structs,
  lists, arrays and maps (temporal, decimal, categorical, enum) raise "Unsupported target
  dtype" (conform.rs:374).
- A target schema with a map dtype has not been compared against a Polars-written map on
  real data; wikidata-pq's tables hold maps as lists of key-value structs.
- The JSON-string (untyped) output takes no target schema.
- Validation covers one official-dump chunk only.
