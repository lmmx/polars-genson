# Polars 2 maps: plan

Polars 2.0 adds `pl.Map(key, value)`, a native map dtype. Map inference is the core of
genson-core, so polars-genson moves to it. This entry records why the first attempt is
being redone, the design that replaces it, and the order of work.

## Background

genson-core decides from the data whether a JSON object is a record (keys are field
names) or a map (keys are data), and writes the decision into the Avro schema as
`{"type": "map", "values": V}`. When it writes a row back out as JSON, `map_encoding`
picks the shape of each map:

| `map_encoding` | JSON shape                                  |
|----------------|---------------------------------------------|
| `"mapping"`    | `{"en": "Hello"}`                           |
| `"entries"`    | `[{"en": "Hello"}]`                         |
| `"kv"`         | `[{"key": "en", "value": "Hello"}]`         |

Polars 1.x has no map dtype. The closest it has is `List(Struct{key, value})`, which
`str.json_decode` builds from `"kv"` JSON, so the Python package defaults to `"kv"` and
reports every map as that list. genson-core and genson-cli default to `"mapping"`.

## The first attempt (PR #216)

PR #216 (branch `claude/polars-map-dtype-compat-20omwz`) kept Polars 1.x support and
made the installed Polars version decide how maps come out of the Python package:

- the reported dtype was `pl.Map` on Polars 2 and `List(Struct{key, value})` on 1.x;
- the `map_encoding` default was `"mapping"` on Polars 2 and `"kv"` on 1.x, because
  Polars 2 only decodes `{"k": v}` JSON into a `pl.Map`;
- typed Parquet wrote a Parquet MAP or a list of structs to match.

A review of wikidata-pq's `process.py` against that branch found a case it cannot
express. `normalise_map_direct` writes typed Parquet with maps as lists of structs,
then checks `avro_to_polars_schema` of the file's Avro schema against an expected
`List(Struct{key, value})`. On Polars 2, `avro_to_polars_schema` returns `pl.Map` with
no way to ask for the list, so the check fails whatever the caller passes. The flaw is
that the data's shape follows `map_encoding` (the caller's choice) while the reported
dtype follows the Polars version (not the caller's choice).

The branch stays as reference material. Parts of it carry over: the bridge's
`Map[String,V]` dtype token, genson-core's native Arrow Map writer
(`avro_to_arrow_type(.., native_map)`), the `schema_to_dict` `pl.Map` case, the
`Categorical(ordering=...)` test fix and the Map tests.

## Design

1. `map_encoding` is the one setting for how a map is represented, and every function
   that reports a dtype or writes data honours it:

   | `map_encoding` | JSON shape     | Polars dtype              | Typed Parquet       |
   |----------------|----------------|---------------------------|---------------------|
   | `"mapping"`    | `{"k": v}`     | `pl.Map(String, V)`       | Parquet MAP         |
   | `"kv"`         | `[{key,value}]`| `List(Struct{key, value})`| list of structs     |
   | `"entries"`    | `[{"k": v}]`   | none — decode and typed output refuse it |

2. polars-genson requires Polars 2. On Polars 1.x, `import polars_genson` fails with a
   message saying so. There is no version branching in the code or the docs.
3. The Python default becomes `"mapping"`, the same as genson-core and genson-cli.
4. `decode=<schema>` takes the map shape from the schema passed: a schema holding a
   `pl.Map` normalises as `"mapping"`, otherwise as `"kv"`, unless `map_encoding` is
   given.
5. Typed Parquet files already record their `map_encoding` in the
   `genson_normalise_config` metadata, so the dtype of a file's contents can be rebuilt
   from the file.

## Stages

Each stage is one branch, one PR and one release, merged before the next starts.

### 1. `map_encoding` on the schema functions (Polars 1.x, no behaviour change)

- polars-jsonschema-bridge emits `Map[String,V]` for an Avro map (and a JSON Schema
  `additionalProperties` object) in place of `List[Struct[key:String,value:V]]`.
- `_parse_polars_dtype` takes `map_encoding` and turns `Map[String,V]` into
  `List(Struct{key, value})` for `"kv"`; `"mapping"` raises, as `pl.Map` needs Polars 2.
- `GensonNamespace.infer_polars_schema` and `avro_to_polars_schema` gain
  `map_encoding`, default `"kv"`. The `infer_polars_schema` expression returns dtype
  strings, not dtypes, so it shows `Map[String,V]` and takes no `map_encoding`.
- Done when: every existing test passes unchanged on Polars 1.44.2, and the bridge
  snapshot and unit tests show the new token.
- Then wikidata-pq passes `map_encoding="kv"` to its two `normalise_from_parquet`
  calls and its `avro_to_polars_schema` call, and one chunk's tables are identical to
  the current release's.

### 2. Polars 2

- `polars>=2.0.0` in the `polars` and `rtcompat` extras and the `dev` group, and a
  re-locked `uv.lock` (`uv sync` re-locks once the bounds change).
- The import guard for Polars 1.x.
- `"mapping"` becomes the default of `normalise_json`, `normalise_from_parquet`,
  `infer_polars_schema`, `avro_to_polars_schema` and the namespace methods, and gives
  `pl.Map`.
- `normalise_from_parquet(typed=True)` accepts `"mapping"` and writes a Parquet MAP.
- `decode=<schema>` follows design point 4.
- `schema_to_dict` renders `pl.Map`.
- Done when: the Python suite passes on Polars 2.0, `just test` and `just clippy`
  pass, and the changelog entry states the new default and the Polars 1.x cut-off.

### 3. Docs

- The maps concept page explains one map as Avro `map`, `pl.Map`, Parquet/Arrow MAP
  and the `kv` list, and how genson moves between them.
- The package READMEs follow.
- The docs site rewrite (branch `docs/site`) merged as #210 on 2026-09-26, so the pages
  are edited in place; `docs/refresh` was an earlier draft of it, closed unmerged.

## Progress

- Stage 1 merged as #217 and released in polars-genson 0.9.7, tested on Polars 1.44.2.
- Stage 2 merged as #218, having passed 262 Python tests on Polars 2.0.0 (3 skipped:
  `coerce_strings` decoding, `entries` decoding, and one map/record test), `cargo test`
  for genson-core (all features), genson-cli and the bridge, `cargo clippy
  --all-targets --all-features -D warnings` and `cargo fmt --check`.
- On Polars 1.44.2 the stage 2 package raises `ImportError` at import.
- `schema_to_json` writes a `pl.Map` as a list of `{key, value}` structs, the Arrow
  storage, as Rust polars 0.55 has no map dtype — `json_to_schema` reads it back as
  that list (polars-genson-py/python/polars_genson/__init__.py `schema_to_json`).
- Stage 3 (branch `map-docs`) adds the Map types page (docs/concepts/map-types.md) and
  updates the pages and READMEs that described maps as `{key, value}` lists; `mkdocs
  build --strict` passes with every example run on Polars 2.0.0.
- The docs build runs its examples against polars-genson from PyPI, not the local
  project (docs/vercel/deploy.sh step 7b) — the site shows `pl.Map` output once stages 2
  and 3 are released.

## Current State

- The Python package defaults `map_encoding` to `"kv"` in `normalise_json`,
  `normalise_from_parquet` and `GensonNamespace.normalise_json`
  (polars-genson-py/python/polars_genson/__init__.py:349, 622, 1090)
- genson-core and genson-cli default `map_encoding` to `"mapping"`
  (genson-core/src/normalise.rs:35, genson-cli/src/main.rs:30)
- polars-jsonschema-bridge converts an Avro map and a JSON Schema
  `additionalProperties` object to the dtype string `List[Struct[key:String,value:V]]`
  (polars-jsonschema-bridge/src/deserialise.rs:111, 167-169)
- `infer_polars_schema` and `avro_to_polars_schema` take no `map_encoding` and report
  every map as `List(Struct{key, value})`
  (polars-genson-py/python/polars_genson/__init__.py:221, 812, 1269)
- `GensonNamespace.normalise_json(decode=...)` raises `NotImplementedError` unless
  `map_encoding="kv"` (polars-genson-py/python/polars_genson/__init__.py:1213-1216)
- `normalise_from_parquet(typed=True)` raises unless `map_encoding="kv"` and writes
  maps as Arrow `List<Struct{key, value}>` (polars-genson-py/src/parquet_io.rs:196-197,
  genson-core/src/parquet.rs:349)
- `normalise_from_parquet` writes its normalise config, including `map_encoding`, to
  the `genson_normalise_config` Parquet metadata key
  (polars-genson-py/src/parquet_io.rs:348)
- polars-genson-py declares `polars>=1.43.2` in the `polars` and `rtcompat` extras and
  the `dev` group (polars-genson-py/pyproject.toml:30, 31, 54)
- PR #216 reported, from its own runs, that Polars 2.0.0 loads the extension built
  against Rust polars 0.55.2, that `str.json_decode` into `pl.Map` rejects `"kv"`
  JSON, and that Polars 1.44.2 reads a Parquet MAP as `List(Struct{key, value})`

## Missing

- The Rust polars 0.55 crates have no Map dtype, so `schema_to_json` cannot write a
  `pl.Map` and the plugin cannot return one directly
- CI builds wheels but runs no Python tests
