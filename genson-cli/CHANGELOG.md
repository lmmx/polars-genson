# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.5...genson-cli-v0.7.0) - 2025-10-10

### <!-- 1 -->Features

- use anstream for all (e)println calls to make them pipeable ([#158](https://github.com/lmmx/polars-genson/pull/158))

## [0.6.5](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.4...genson-cli-v0.6.5) - 2025-10-09

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.6.4](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.3...genson-cli-v0.6.4) - 2025-10-09

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.6.3](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.2...genson-cli-v0.6.3) - 2025-10-08

### <!-- 9 -->Other

- Specify fields to enforce scalar promotion for ([#155](https://github.com/lmmx/polars-genson/pull/155))

## [0.6.2](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.1...genson-cli-v0.6.2) - 2025-10-08

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.6.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.6.0...genson-cli-v0.6.1) - 2025-10-08

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.6.0](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.5...genson-cli-v0.6.0) - 2025-10-08

### <!-- 9 -->Other

- update Cargo.lock dependencies

## [0.5.5](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.4...genson-cli-v0.5.5) - 2025-10-04

### <!-- 9 -->Other

- control builder parallelism ([#138](https://github.com/lmmx/polars-genson/pull/138))

## [0.5.4](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.3...genson-cli-v0.5.4) - 2025-10-04

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.5.3](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.2...genson-cli-v0.5.3) - 2025-10-03

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.5.2](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.1...genson-cli-v0.5.2) - 2025-10-03

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.5.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.5.0...genson-cli-v0.5.1) - 2025-10-03

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.5.0](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.7...genson-cli-v0.5.0) - 2025-10-02

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.4.7](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.6...genson-cli-v0.4.7) - 2025-10-02

### <!-- 9 -->Other

- Add `profile` (timing logs) flag ([#119](https://github.com/lmmx/polars-genson/pull/119))

## [0.4.4](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.3...genson-cli-v0.4.4) - 2025-10-01

### <!-- 2 -->Bug Fixes

- fix anyOf resolution in map inference unification ([#115](https://github.com/lmmx/polars-genson/pull/115))

## [0.4.3](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.2...genson-cli-v0.4.3) - 2025-09-30

### <!-- 9 -->Other

- Control record unification with `--no-unify`; L14 repro ([#114](https://github.com/lmmx/polars-genson/pull/114))

## [0.4.2](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.1...genson-cli-v0.4.2) - 2025-09-25

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.4.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.4.0...genson-cli-v0.4.1) - 2025-09-25

### <!-- 9 -->Other

- update Cargo.lock dependencies

## [0.3.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.3.0...genson-cli-v0.3.1) - 2025-09-25

### <!-- 9 -->Other

- exclude development files from crate release
- Resolve `anyOf` unions before map inference ([#108](https://github.com/lmmx/polars-genson/pull/108))
- add minimal fixtures to reproduce map inference failures ([#107](https://github.com/lmmx/polars-genson/pull/107))

## [0.2.8](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.7...genson-cli-v0.2.8) - 2025-09-20

### <!-- 6 -->Testing

- test `--unify-map` in combination with `--map-threshold` ([#96](https://github.com/lmmx/polars-genson/pull/96))

### <!-- 9 -->Other

- harmonise synthetic key for object-promoted scalars in map unification and normalisation ([#100](https://github.com/lmmx/polars-genson/pull/100))
- unify scalars as well as records ([#98](https://github.com/lmmx/polars-genson/pull/98))
- prevent root map ([#97](https://github.com/lmmx/polars-genson/pull/97))

## [0.2.7](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.6...genson-cli-v0.2.7) - 2025-09-17

### <!-- 9 -->Other

- respect `--map-max-rk` in `--unify-maps` mode ([#94](https://github.com/lmmx/polars-genson/pull/94))

## [0.2.6](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.5...genson-cli-v0.2.6) - 2025-09-17

### <!-- 9 -->Other

- schema unification upgrades ([#93](https://github.com/lmmx/polars-genson/pull/93))

## [0.2.5](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.4...genson-cli-v0.2.5) - 2025-09-17

### <!-- 9 -->Other

- updated the following local packages: genson-core

## [0.2.4](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.3...genson-cli-v0.2.4) - 2025-09-17

### <!-- 6 -->Testing

- extract more fixtures from the 4 row claims JSONL ([#88](https://github.com/lmmx/polars-genson/pull/88))

### <!-- 9 -->Other

- debug logs ([#89](https://github.com/lmmx/polars-genson/pull/89))

## [0.2.3](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.2...genson-cli-v0.2.3) - 2025-09-16

### <!-- 6 -->Testing

- bless array unification snapshots ([#85](https://github.com/lmmx/polars-genson/pull/85))

### <!-- 9 -->Other

- strengthen type unification ([#86](https://github.com/lmmx/polars-genson/pull/86))

## [0.2.2](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.1...genson-cli-v0.2.2) - 2025-09-16

### <!-- 5 -->Refactor

- simplify claims fixture ([#82](https://github.com/lmmx/polars-genson/pull/82))

### <!-- 9 -->Other

- map unify array of records ([#83](https://github.com/lmmx/polars-genson/pull/83))

## [0.2.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.2.0...genson-cli-v0.2.1) - 2025-09-16

### <!-- 6 -->Testing

- snapshot incompatible record results ([#81](https://github.com/lmmx/polars-genson/pull/81))
- bless the test cases for map of unified records ([#80](https://github.com/lmmx/polars-genson/pull/80))

### <!-- 9 -->Other

- unify map of union of records ([#79](https://github.com/lmmx/polars-genson/pull/79))

## [0.1.9](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.8...genson-cli-v0.1.9) - 2025-09-11

### <!-- 9 -->Other

- map max required keys ([#68](https://github.com/lmmx/polars-genson/pull/68))

## [0.1.8](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.7...genson-cli-v0.1.8) - 2025-09-10

### <!-- 1 -->Features

- support NDJSON root wrapping ([#64](https://github.com/lmmx/polars-genson/pull/64))

## [0.1.7](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.6...genson-cli-v0.1.7) - 2025-09-10

### <!-- 1 -->Features

- option to wrap JSON root in column name field ([#63](https://github.com/lmmx/polars-genson/pull/63))

### <!-- 6 -->Testing

- identify issue with map of struct ([#61](https://github.com/lmmx/polars-genson/pull/61))

## [0.1.6](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.5...genson-cli-v0.1.6) - 2025-09-09

### <!-- 1 -->Features

- *(map-encoding)* map normalisation encodings ([#59](https://github.com/lmmx/polars-genson/pull/59))

## [0.1.3](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.2...genson-cli-v0.1.3) - 2025-09-06

### <!-- 4 -->Documentation

- *(mkdocs)* new config [ported from page-dewarp project] ([#44](https://github.com/lmmx/polars-genson/pull/44))

### <!-- 9 -->Other

- schema map inference ([#49](https://github.com/lmmx/polars-genson/pull/49))

## [0.1.2](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.1...genson-cli-v0.1.2) - 2025-08-20

### <!-- 4 -->Documentation

- give all crates decent READMEs ([#14](https://github.com/lmmx/polars-genson/pull/14))

## [0.1.1](https://github.com/lmmx/polars-genson/compare/genson-cli-v0.1.0...genson-cli-v0.1.1) - 2025-08-20

### <!-- 9 -->Other

- preserve order ([#11](https://github.com/lmmx/polars-genson/pull/11))

## [0.1.0](https://github.com/lmmx/polars-genson/releases/tag/genson-cli-v0.1.0) - 2025-08-20

### <!-- 8 -->Styling

- lint

### <!-- 9 -->Other

- amend release process and do a dry run
- new package and version bumps ([#5](https://github.com/lmmx/polars-genson/pull/5))
- initial setup of working Rust/Python polars plugin
