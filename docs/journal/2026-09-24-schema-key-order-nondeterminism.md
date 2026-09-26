# 2026-09-24 schema key order differs between runs since 0.6.0

## Current State

- `infer_json_schema` returns the same schema content with a different dict key order on repeated runs of the same input in polars-genson 0.7.4, 0.7.5, 0.7.6 (4 runs each on `chunk_2-00114-of-00144.parquet` `claims`, 4 distinct raw-order hashes per version, one shared hash with keys sorted)
- The order is stable across runs at genson-core `ca2d514` (py-0.5.5), `efcf3c9`, `f10ea87` and `706f68f`, and varies across runs at `abdeb62` (simd-json 0.13.10 to 0.17.0, #147) and `f5454fc` (py-0.6.0) — bisected by building `genson-core` at each commit and running a 1500-row `claims` slice 4 times
- simd-json 0.17.0 parses objects into `halfbrown::HashMap` 0.4.0, whose default hasher is `hashbrown`'s randomly seeded one, and objects of more than 32 keys switch from vector storage to that hash map — `ObjectStrategy::add_object` inserts `properties` in `object.iter()` order (genson-core/src/genson_rs/strategy/object.rs)
- Adding `halfbrown = { version = "0.4", features = ["fxhash"] }` to `genson-core/Cargo.toml` at `abdeb62` gave 1 distinct output in 6 runs, while enabling simd-json's `known-key` feature gave 6 distinct outputs in 6 runs
- Commits `cfa26c6` (#143) and `db6afe3` (#146) in the 0.5.5 to 0.6.0 window record an unresolved schema non-determinism ("still not found source of randomness", "isolate JSON non-determinism bug further"), and `abdeb62` disables the `x4-L1` fixture as unstable
- 2026-09-26, on a build of master at `c50b6c8`: one row `{"m": {"k0": {"common": 0, "f0": "s"}, …}}` with K keys in `m`, inferred with `map_threshold=5, unify_maps=True`, gives `m` a map whose value record lists `common` then `f0 … f{K-1}` — with K=10 the fields are in document order (`f0, f1, …, f9`) in 6 of 6 separate processes, and with K=40 each of 6 separate processes gives a different order.
- The merged value record's field order follows the order in which the map's keys are visited, so the >32-key hash-map iteration in simd-json objects reaches genson's output through map-value unification as well as through `ObjectStrategy::add_object`.
- 2026-09-26, on the same build: `infer_json_schema` on `chunk_2-00114` claims with the wikidata-pq options gives one raw key order in 6 of 6 separate processes, and on `chunk_0-00004` the `mainsnak.datavalue` field order of `normalise_from_parquet(typed=True)` differed between separate runs on one machine (at least five distinct orders across ten outputs from six runs).
- Wikidata claims maps are keyed by property id, one key per property an entity has, so entities with more than 32 properties give claims objects of more than 32 keys.
- On `fix/key-order-determinism`, genson_rs reads each parsed value from simd-json's tape (`simd_json::to_tape`, `simd_json::tape::Value`), whose `Object::iter` walks keys in document order for every object size, instead of from `BorrowedValue` (genson-core/src/genson_rs/mod.rs, builder.rs, node.rs, strategy/*.rs).
- On that branch, `test_wide_object_keeps_key_order` (genson-core/tests/key_order.rs, polars-genson-py/tests/polars_schema_test.py) passes: an object with keys `z … a, Z … A` (52 keys) infers its properties in that order, and the same test gives `c, b, o, Z, z, …` on master.
- On that branch, the K=40 map repro above gives document order (`f0 … f39`) in 3 of 3 separate processes, and the genson-cli snapshot suite has no changes.
- `genson-cli` release builds on `x1818_L26.jsonl` (15 MB), best of 7: inference 0.205 s on master against 0.195 s on the branch, and inference with `--avro --normalise --unify-maps --map-threshold 0` 0.807 s against 0.809 s.

## Missing

- A check of `chunk_0-00004` field order across repeated runs on the tape-based build.
