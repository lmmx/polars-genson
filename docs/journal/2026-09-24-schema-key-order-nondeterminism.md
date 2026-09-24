# 2026-09-24 schema key order differs between runs since 0.6.0

## Current State

- `infer_json_schema` returns the same schema content with a different dict key order on repeated runs of the same input in polars-genson 0.7.4, 0.7.5, 0.7.6 (4 runs each on `chunk_2-00114-of-00144.parquet` `claims`, 4 distinct raw-order hashes per version, one shared hash with keys sorted)
- The order is stable across runs at genson-core `ca2d514` (py-0.5.5), `efcf3c9`, `f10ea87` and `706f68f`, and varies across runs at `abdeb62` (simd-json 0.13.10 to 0.17.0, #147) and `f5454fc` (py-0.6.0) — bisected by building `genson-core` at each commit and running a 1500-row `claims` slice 4 times
- simd-json 0.17.0 parses objects into `halfbrown::HashMap` 0.4.0, whose default hasher is `hashbrown`'s randomly seeded one, and objects of more than 32 keys switch from vector storage to that hash map — `ObjectStrategy::add_object` inserts `properties` in `object.iter()` order (genson-core/src/genson_rs/strategy/object.rs)
- Adding `halfbrown = { version = "0.4", features = ["fxhash"] }` to `genson-core/Cargo.toml` at `abdeb62` gave 1 distinct output in 6 runs, while enabling simd-json's `known-key` feature gave 6 distinct outputs in 6 runs
- Commits `cfa26c6` (#143) and `db6afe3` (#146) in the 0.5.5 to 0.6.0 window record an unresolved schema non-determinism ("still not found source of randomness", "isolate JSON non-determinism bug further"), and `abdeb62` disables the `x4-L1` fixture as unstable

## Missing

- The `fxhash` feature is not applied on master
- No comparison of the schema produced with `fxhash` against the 0.5.5 schema for the same input
- No check of the `fxhash` fix on current master
