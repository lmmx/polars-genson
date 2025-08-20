# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/lmmx/polars-genson/releases/tag/polars-genson-py-v0.1.0) - 2025-08-20

### <!-- 1 -->Features

- Polars schemas from JSON column ([#9](https://github.com/lmmx/polars-genson/pull/9))
- working `list[struct["name","dtype"]]` Series ([#8](https://github.com/lmmx/polars-genson/pull/8))
- working MVP :tada: ([#3](https://github.com/lmmx/polars-genson/pull/3))
- option for per-row schemas or all batched together

### <!-- 4 -->Documentation

- dev docs hint
- highlight

### <!-- 5 -->Refactor

- *(bridge)* Polars-JSON conversion code ([#10](https://github.com/lmmx/polars-genson/pull/10))
- *(rename)* 'infer_schema' to 'infer_json_schema' ([#6](https://github.com/lmmx/polars-genson/pull/6))

### <!-- 8 -->Styling

- lint

### <!-- 9 -->Other

- amend release process and do a dry run
- tidier linting recipes
- new package and version bumps ([#5](https://github.com/lmmx/polars-genson/pull/5))
- appease linters
- initial setup of working Rust/Python polars plugin
