# 2026-09-24 infer_json_schema profile on Wikidata claims

## Current State

- `infer_json_schema` 0.7.6 on `chunk_3-00031-of-00064.parquet` (4851 rows, 246.7 MB `claims` JSON, `max_builders=100`) takes 3.14s wall in `genson-core/examples/bench_claims.rs` on a 10-core/20-thread i9-10900K, 3.22s through the Python plugin — 0.7.5 takes 23.11s and 0.7.4 takes 35.48s through the Python plugin, one job at a time, single run each
- Summed across worker threads, the per-row stages of that 3.14s run cost 0.52 CPU-s validation (`prepare_json_bytes`), 4.57 CPU-s build (`build_json_schema_into`), 18.39 CPU-s `to_schema` plus builder drop, 0.55 CPU-s `apply_force_field_types`, 1.18 CPU-s xxh64 of the serialised schema (genson-core/src/schema.rs)
- `SchemaNode::to_schema` cloned the child schema of every single-strategy node, `SchemaBuilder::to_schema` cloned every root key, and `ObjectStrategy::properties_to_schema` built its map through `json!` and index-assignment — commit c06e892 moves the values instead, and the same run takes 2.24s wall with `to_schema` at 5.2 CPU-s (genson-core/src/genson_rs/node.rs, builder.rs, strategy/object.rs)
- Schema output of the 4851-row run is unchanged by c06e892 — the schema hash printed by `bench_claims` is `c7f5f7c6ee619612` before and after, `schema_len` 259027, and `cargo test -p genson-core --release` passes
- Leaking each merged row schema instead of freeing it (ablation) cut the serial merge from about 1270 ms to about 880 ms summed over 49 chunks — freeing the per-row `serde_json::Value` inside `SchemaBuilder::add_schema` accounts for about 390 ms of the merge, so the next commit merges by reference via `SchemaBuilder::add_schema_mut` and drops each chunk's schemas across the rayon pool (genson-core/src/genson_rs/builder.rs, genson-core/src/schema.rs)
- With that change the `bench_claims` run takes 1.88–1.91s wall against 2.24s, the summed serial merge (now including the parallel drop) is about 960 ms, and the schema hash stays `c7f5f7c6ee619612`
- After c06e892 the `profile=True` chunk loop splits into 767 ms summed parallel phase and 1223 ms summed serial-merge phase over 49 chunks — the serial merge (`SchemaBuilder::add_schema`) averages 0.2 ms per unique row schema at about 34 KB per schema
- Across 7 Wikidata chunk files (0.014–0.098 GB parquet) 0.7.6 fits 0.9s + 33 s/GB, 0.7.5 fits 5.9s + 174 s/GB, 0.7.4 fits 7.7s + 276 s/GB — applied to the 9687 files / 1656.3 GB in `scripts/source_size` of the wikidata repo that is about 18 h, 96 h, 148 h single-threaded, measured before c06e892
- `bench_claims` reads an NDJSON file with one row per line (dumped from the parquet `claims` column), runs `infer_json_schema_from_strings` with `delimiter=b'\n'` and prints wall time and the schema hash (genson-core/examples/bench_claims.rs)

### Measured and not kept

- Removing `add_extra_keywords` from `ObjectStrategy::add_schema` cut the summed serial merge from about 1280 ms to about 1215 ms, and removing the `required` intersection cut it to about 1115 ms — skipping the `required` intersection once the set is empty (`!req.is_empty()`) left the merge at about 960 ms, unchanged

- Merging chunk N on a separate thread while rayon builds chunk N+1 gave 3.2s against 3.14s before c06e892 (serial merge doubled to 60–86 ms per chunk while overlapped) and 2.04s against 2.15s after the `to_schema` fix, with identical output
- Skipping the deep clone of `properties` and `required` into the extra keywords in `ObjectStrategy::add_schema` gave 3.03–3.12s against 3.14s with identical output once the keys' positions were preserved, and changed key order without that
- Splitting `anyOf` and type arrays through a `needs_splitting` check before `get_subschemas` gave 3.04–3.08s against 3.14s with identical output
- Raising `PARALLEL_PROCESSING_BOUNDARY` in genson-core/src/genson_rs/strategy/array.rs to 1000000 gave 3.03–3.06s against 3.14s
- Hashing each top-level property subschema of the merged row schemas found 21 KB duplicated out of 165 MB, so identical-subschema skipping applies to almost none of the merge work
- Merging each chunk as a `fold`/`reduce` of `SchemaBuilder`s produced 259146 bytes of schema against 259027 and took 13.5s
- `pprof` sampling produced no resolved frames in this container (`perf_event_paranoid=4`, no `perf`, `samply`, `gdb` or `valgrind` installed)

## Missing

- `chunk_0-00004` (0.615 GB) was killed (rc=137) in this container on every version, so no timing exists for files above 0.098 GB
- No allocation counts per stage
- No re-fit of the full-dataset estimate after c06e892
