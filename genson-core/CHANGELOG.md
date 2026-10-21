# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0](https://github.com/lmmx/polars-genson/compare/genson-core-v1.1.0...genson-core-v1.2.0) - 2026-10-21

### <!-- 1 -->Features

- typed Parquet output in a target schema (schema, schema_check, unnest) ([#287](https://github.com/lmmx/polars-genson/pull/287))

## [1.1.0](https://github.com/lmmx/polars-genson/compare/genson-core-v1.0.2...genson-core-v1.1.0) - 2026-10-19

### <!-- 2 -->Bug Fixes

- *(core)* a union of a list and other types is one list holding their values ([#262](https://github.com/lmmx/polars-genson/pull/262))
- *(core)* an empty list beside scalars takes their type as its items ([#261](https://github.com/lmmx/polars-genson/pull/261))
- *(core)* a value where the schema types null normalises to null ([#265](https://github.com/lmmx/polars-genson/pull/265))
- *(core)* a forced map keeps its values' type when it is a scalar in some rows ([#264](https://github.com/lmmx/polars-genson/pull/264))
- *(core)* a map of null values unifies with one of typed values as nullable ([#263](https://github.com/lmmx/polars-genson/pull/263))
- *(core)* a value in a union of null alone normalises to null, not a panic ([#260](https://github.com/lmmx/polars-genson/pull/260))

### <!-- 3 -->Performance

- normalise_json(decode=False) writes its JSON from the typed decoder ([#269](https://github.com/lmmx/polars-genson/pull/269))
- *(core)* pad a record's absent fields with nulls in bulk, when next written ([#268](https://github.com/lmmx/polars-genson/pull/268))
- *(core)* unescape strings directly rather than through serde_json ([#266](https://github.com/lmmx/polars-genson/pull/266))

### <!-- 9 -->Other

- *(deps)* bump avrotize to 0.1.2 ([#270](https://github.com/lmmx/polars-genson/pull/270))

## [1.0.2](https://github.com/lmmx/polars-genson/compare/genson-core-v1.0.1...genson-core-v1.0.2) - 2026-10-19

### <!-- 2 -->Bug Fixes

- *(core)* an integer beyond 64 bits is a number, not a panic ([#235](https://github.com/lmmx/polars-genson/pull/235))
- *(core)* map unification requires a field only when every record requires it ([#232](https://github.com/lmmx/polars-genson/pull/232))

### <!-- 3 -->Performance

- *(core)* a property given an earlier sibling's states shares its subtree ([#254](https://github.com/lmmx/polars-genson/pull/254))
- *(core)* write the DAG without allocating or hashing known terms per node ([#253](https://github.com/lmmx/polars-genson/pull/253))
- *(core)* rewrite the merged schema on a hash-consed DAG ([#252](https://github.com/lmmx/polars-genson/pull/252))
- *(core)* merge interned rows with each property's states absorbed in parallel ([#251](https://github.com/lmmx/polars-genson/pull/251))
- *(core)* intern a row's values without allocating or hashing what repeats ([#250](https://github.com/lmmx/polars-genson/pull/250))
- *(core)* read rows with braces in strings as the usual path splits them ([#249](https://github.com/lmmx/polars-genson/pull/249))
- *(core)* encode each row group's columns in parallel ([#248](https://github.com/lmmx/polars-genson/pull/248))
- *(core)* unification moves the schema so far into the result ([#247](https://github.com/lmmx/polars-genson/pull/247))
- *(core)* infer the rows' schemas as interned states ([#246](https://github.com/lmmx/polars-genson/pull/246))
- *(core)* normalise rows of JSON text straight into Arrow ([#245](https://github.com/lmmx/polars-genson/pull/245))
- *(core)* normalise against a plan compiled from the schema once ([#244](https://github.com/lmmx/polars-genson/pull/244))
- *(core)* merge rows' schemas without rebuilding them, and write large objects in parallel ([#243](https://github.com/lmmx/polars-genson/pull/243))
- *(core)* move values into the JSON objects that inference and normalisation build ([#242](https://github.com/lmmx/polars-genson/pull/242))
- *(parquet)* decode a string column with several readers at once ([#241](https://github.com/lmmx/polars-genson/pull/241))
- *(core)* extract invariant fields by scanning each row's text ([#240](https://github.com/lmmx/polars-genson/pull/240))
- *(parquet)* read only the requested column, and borrow its rows ([#239](https://github.com/lmmx/polars-genson/pull/239))
- *(core)* write Parquet output with zstd compression ([#237](https://github.com/lmmx/polars-genson/pull/237))
- *(core)* normalise takes record fields out of their input without prune ([#236](https://github.com/lmmx/polars-genson/pull/236))
- *(core)* merge a bare scalar without building its schema ([#234](https://github.com/lmmx/polars-genson/pull/234))
- *(core)* debug macros build their message only when it prints ([#233](https://github.com/lmmx/polars-genson/pull/233))
- *(core)* hash scalar leaves without building their schema value ([#226](https://github.com/lmmx/polars-genson/pull/226))

### <!-- 7 -->Build System and CI

- a performance scorecard of genson-core's stages for pull requests ([#255](https://github.com/lmmx/polars-genson/pull/255))

### <!-- 9 -->Other

- *(clippy)* fix lint

## [1.0.1](https://github.com/lmmx/polars-genson/compare/genson-core-v1.0.0...genson-core-v1.0.1) - 2026-10-15

### <!-- 2 -->Bug Fixes

- fix/add schemas merge existing properties ([#224](https://github.com/lmmx/polars-genson/pull/224))

### <!-- 3 -->Performance

- perf/schema merge no clones ([#223](https://github.com/lmmx/polars-genson/pull/223))

## [1.0.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.9.4...genson-core-v1.0.0) - 2026-10-03

### <!-- 2 -->Bug Fixes

- *(map-inference)* keep forced maps' value schema when merging per-string schemas ([#215](https://github.com/lmmx/polars-genson/pull/215))

### <!-- 9 -->Other

- lint

## [0.9.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.9.3...genson-core-v0.9.4) - 2026-10-03

### <!-- 2 -->Bug Fixes

- *(map-inference)* an always-empty map candidate is a map of null, not an empty record ([#214](https://github.com/lmmx/polars-genson/pull/214))

### <!-- 9 -->Other

- lint

## [0.9.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.9.2...genson-core-v0.9.3) - 2026-10-03

### <!-- 2 -->Bug Fixes

- *(map-inference)* unify empty arrays with any array; keep objects holding a force-promoted field as records ([#213](https://github.com/lmmx/polars-genson/pull/213))

### <!-- 9 -->Other

- prune ([#212](https://github.com/lmmx/polars-genson/pull/212))

## [0.9.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.9.1...genson-core-v0.9.2) - 2026-09-27

### <!-- 2 -->Bug Fixes

- *(core)* promote scalar list items among records when unifying map values ([#211](https://github.com/lmmx/polars-genson/pull/211))

## [0.9.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.9.0...genson-core-v0.9.1) - 2026-09-26

### <!-- 2 -->Bug Fixes

- fix/forced map value type ([#209](https://github.com/lmmx/polars-genson/pull/209))

## [0.9.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.8.0...genson-core-v0.9.0) - 2026-09-26

### <!-- 9 -->Other

- Parquet normalisation fixes ([#196](https://github.com/lmmx/polars-genson/pull/196))

## [0.8.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.5...genson-core-v0.8.0) - 2026-09-24

### <!-- 2 -->Bug Fixes

- respect no_root_map in the map-of-records branch ([#192](https://github.com/lmmx/polars-genson/pull/192))

### <!-- 3 -->Performance

- pass borrowed strings from the Python plugin instead of copying the column ([#193](https://github.com/lmmx/polars-genson/pull/193))

## [0.7.5](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.4...genson-core-v0.7.5) - 2026-09-24

### <!-- 3 -->Performance

- perf/genson core speedups 3 ([#191](https://github.com/lmmx/polars-genson/pull/191))

## [0.7.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.3...genson-core-v0.7.4) - 2026-09-24

### <!-- 3 -->Performance

- perf/genson core speedups 2 ([#190](https://github.com/lmmx/polars-genson/pull/190))
- perf/genson core profiling ([#189](https://github.com/lmmx/polars-genson/pull/189))

## [0.7.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.2...genson-core-v0.7.3) - 2026-09-24

### <!-- 9 -->Other

- Speed up `normalise_from_parquet` (4.5x faster) ([#188](https://github.com/lmmx/polars-genson/pull/188))

## [0.7.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.1...genson-core-v0.7.2) - 2026-09-24

### <!-- 2 -->Bug Fixes

- map inference fix from Wikidata claims fixtures ([#175](https://github.com/lmmx/polars-genson/pull/175))

### <!-- 3 -->Performance

- convert each row's builder to a schema on the worker thread ([#186](https://github.com/lmmx/polars-genson/pull/186))
- benchmark genson-rs perf ([#176](https://github.com/lmmx/polars-genson/pull/176))

### <!-- 9 -->Other

- chore/clippy 1.98 ([#187](https://github.com/lmmx/polars-genson/pull/187))
- tidier debug ([#174](https://github.com/lmmx/polars-genson/pull/174))

## [0.7.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.7.0...genson-core-v0.7.1) - 2025-10-12

### <!-- 4 -->Documentation

- update all Readmes

## [0.7.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.5...genson-core-v0.7.0) - 2025-10-10

### <!-- 1 -->Features

- use anstream for all (e)println calls to make them pipeable ([#158](https://github.com/lmmx/polars-genson/pull/158))

## [0.6.5](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.4...genson-core-v0.6.5) - 2025-10-09

### <!-- 2 -->Bug Fixes

- apply forced scalar promo to nullable scalars ([#157](https://github.com/lmmx/polars-genson/pull/157))

## [0.6.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.3...genson-core-v0.6.4) - 2025-10-09

### <!-- 9 -->Other

- enforce scalar promotion fields in map unification too ([#156](https://github.com/lmmx/polars-genson/pull/156))

## [0.6.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.2...genson-core-v0.6.3) - 2025-10-08

### <!-- 9 -->Other

- Specify fields to enforce scalar promotion for ([#155](https://github.com/lmmx/polars-genson/pull/155))

## [0.6.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.1...genson-core-v0.6.2) - 2025-10-08

### <!-- 1 -->Features

- *(parquet)* write schema metadata when normalising directly to disk ([#152](https://github.com/lmmx/polars-genson/pull/152))

## [0.6.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.6.0...genson-core-v0.6.1) - 2025-10-08

### <!-- 9 -->Other

- Support writing large string parquet columns ([#151](https://github.com/lmmx/polars-genson/pull/151))

## [0.6.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.5...genson-core-v0.6.0) - 2025-10-08

### <!-- 9 -->Other

- update simd-json to latest; disable unstable x4-L1 fixture ([#147](https://github.com/lmmx/polars-genson/pull/147))

## [0.5.5](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.4...genson-core-v0.5.5) - 2025-10-04

### <!-- 9 -->Other

- control builder parallelism ([#138](https://github.com/lmmx/polars-genson/pull/138))

## [0.5.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.3...genson-core-v0.5.4) - 2025-10-04

### <!-- 3 -->Performance

- profile the peak RSS and move other profile logs to verbose ([#137](https://github.com/lmmx/polars-genson/pull/137))

## [0.5.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.2...genson-core-v0.5.3) - 2025-10-03

### <!-- 3 -->Performance

- reduce clones in unification functions ([#135](https://github.com/lmmx/polars-genson/pull/135))

## [0.5.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.1...genson-core-v0.5.2) - 2025-10-03

### <!-- 3 -->Performance

- reduce JSON string cloning (closes #122) ([#132](https://github.com/lmmx/polars-genson/pull/132))

## [0.5.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.5.0...genson-core-v0.5.1) - 2025-10-03

### <!-- 2 -->Bug Fixes

- gate the profile logs (too noisy)

## [0.5.0](https://github.com/lmmx/polars-genson/compare/genson-core-v0.4.6...genson-core-v0.5.0) - 2025-10-02

### <!-- 3 -->Performance

- minor array reordering speedup (not a bottleneck)

### <!-- 9 -->Other

- Parallelise/batch schema merge ([#120](https://github.com/lmmx/polars-genson/pull/120))

## [0.4.6](https://github.com/lmmx/polars-genson/compare/genson-core-v0.4.5...genson-core-v0.4.6) - 2025-10-02

### <!-- 9 -->Other

- Add `profile` (timing logs) flag ([#119](https://github.com/lmmx/polars-genson/pull/119))
- Parallelise JSON string prep and schema building ([#118](https://github.com/lmmx/polars-genson/pull/118))

## [0.4.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.4.2...genson-core-v0.4.3) - 2025-10-01

### <!-- 2 -->Bug Fixes

- fix anyOf resolution in map inference unification ([#115](https://github.com/lmmx/polars-genson/pull/115))

## [0.4.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.4.1...genson-core-v0.4.2) - 2025-09-30

### <!-- 9 -->Other

- Control record unification with `--no-unify`; L14 repro ([#114](https://github.com/lmmx/polars-genson/pull/114))

## [0.4.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.4.0...genson-core-v0.4.1) - 2025-09-25

### <!-- 9 -->Other

- scalar union promotion ([#111](https://github.com/lmmx/polars-genson/pull/111))

## [0.2.8](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.7...genson-core-v0.2.8) - 2025-09-20

### <!-- 9 -->Other

- harmonise synthetic key for object-promoted scalars in map unification and normalisation ([#100](https://github.com/lmmx/polars-genson/pull/100))
- unify scalars as well as records ([#98](https://github.com/lmmx/polars-genson/pull/98))
- prevent root map ([#97](https://github.com/lmmx/polars-genson/pull/97))

## [0.2.7](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.6...genson-core-v0.2.7) - 2025-09-17

### <!-- 9 -->Other

- respect `--map-max-rk` in `--unify-maps` mode ([#94](https://github.com/lmmx/polars-genson/pull/94))

## [0.2.6](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.5...genson-core-v0.2.6) - 2025-09-17

### <!-- 9 -->Other

- schema unification upgrades ([#93](https://github.com/lmmx/polars-genson/pull/93))
- remove leftover comments

## [0.2.5](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.4...genson-core-v0.2.5) - 2025-09-17

### <!-- 5 -->Refactor

- refactor tests into included modules ([#91](https://github.com/lmmx/polars-genson/pull/91))

### <!-- 9 -->Other

- modularise the schema handling ([#92](https://github.com/lmmx/polars-genson/pull/92))

## [0.2.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.3...genson-core-v0.2.4) - 2025-09-17

### <!-- 6 -->Testing

- extract more fixtures from the 4 row claims JSONL ([#88](https://github.com/lmmx/polars-genson/pull/88))

### <!-- 9 -->Other

- debug logs ([#89](https://github.com/lmmx/polars-genson/pull/89))

## [0.2.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.2...genson-core-v0.2.3) - 2025-09-16

### <!-- 9 -->Other

- strengthen type unification ([#86](https://github.com/lmmx/polars-genson/pull/86))

## [0.2.2](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.1...genson-core-v0.2.2) - 2025-09-16

### <!-- 9 -->Other

- map unify array of records ([#83](https://github.com/lmmx/polars-genson/pull/83))

## [0.2.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.2.0...genson-core-v0.2.1) - 2025-09-16

### <!-- 9 -->Other

- unify map of union of records ([#79](https://github.com/lmmx/polars-genson/pull/79))

## [0.1.10](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.9...genson-core-v0.1.10) - 2025-09-11

### <!-- 9 -->Other

- map max required keys ([#68](https://github.com/lmmx/polars-genson/pull/68))

## [0.1.9](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.8...genson-core-v0.1.9) - 2025-09-10

### <!-- 1 -->Features

- support NDJSON root wrapping ([#64](https://github.com/lmmx/polars-genson/pull/64))

## [0.1.8](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.7...genson-core-v0.1.8) - 2025-09-10

### <!-- 1 -->Features

- option to wrap JSON root in column name field ([#63](https://github.com/lmmx/polars-genson/pull/63))

## [0.1.7](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.6...genson-core-v0.1.7) - 2025-09-09

### <!-- 1 -->Features

- *(map-encoding)* map normalisation encodings ([#59](https://github.com/lmmx/polars-genson/pull/59))

## [0.1.4](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.3...genson-core-v0.1.4) - 2025-09-06

### <!-- 1 -->Features

- use OrderMap for equality comparison ([#43](https://github.com/lmmx/polars-genson/pull/43))

### <!-- 4 -->Documentation

- docs background ([#47](https://github.com/lmmx/polars-genson/pull/47))

### <!-- 9 -->Other

- schema map inference ([#49](https://github.com/lmmx/polars-genson/pull/49))

## [0.1.3](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.2...genson-core-v0.1.3) - 2025-08-20

### <!-- 4 -->Documentation

- give all crates decent READMEs ([#14](https://github.com/lmmx/polars-genson/pull/14))

## [0.1.1](https://github.com/lmmx/polars-genson/compare/genson-core-v0.1.0...genson-core-v0.1.1) - 2025-08-20

### <!-- 9 -->Other

- amend release process and do a dry run
