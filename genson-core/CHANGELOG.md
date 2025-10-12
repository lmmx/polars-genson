# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
