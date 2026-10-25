# 2026-10-25: Decoding into the target's 64-bit offsets (tried, not shipped)

With a target `schema`, the typed decoder was made to build the target's `LargeUtf8` and
`LargeList` columns directly, so that `conform` would have no offsets to widen (#314,
branch `perf/large-offsets`). The output was the same, but the change saved no time and
raised the worker's peak RSS by 17%. It is not shipped.

## Why

- **Two passes over the offsets:** genson decodes strings and lists with 32-bit offsets
  (`Utf8`, `List`, the types `avro_record_fields` gives). A target schema's strings and
  lists have 64-bit ones (`data_type_from_spec`: `LargeUtf8`, `LargeList`, as Polars
  writes them).
- **What conform does:** it casts each string column (`arrow::compute::cast`, which
  writes a new offsets buffer and shares the values) and widens each list's offsets
  (`widen`).
- **The estimate:** a profile of wikidata-pq's claims call after #311 put those casts at
  about 3% of the call's user CPU, plus the page faults of the new buffers.
- **Locally:** `conform` took 0.04–0.07 s of the claims call's 0.8 s wall time on chunk
  c0, and 0.12–0.40 s of 0.7–2.3 s on s90 (1.5.0, a shared 4-core VM, very noisy).

## The change

- **The decode type:** `conform::with_target_offsets` gave the inferred type with 64-bit
  offsets wherever the target had them, through struct fields (by name), list items and
  map keys and values.
- **The decoder:** its columns (`normalise/typed.rs`) held 32- or 64-bit string builders
  and list offsets (`Strings`, `Offsets`). The limits that send a batch to the `Value`
  path were unchanged, so the same batches were decoded and declined.
- **`normalise_from_parquet`:** it decoded into that type both for the strict decode
  against `avro_schema` and for the typed decode. `conform` then found the leaves
  already in the target's types and passed them through.

## The output was the same

- **Buffer for buffer:** `test_typed_decode_matches_the_value_path` decoded each of its
  2,500 random cases with 64-bit offsets as well. That declined exactly where the 32-bit
  decode did. Otherwise it matched `conform` of the 32-bit arrays in types, lengths,
  offsets, validity, every buffer and every child, with the same pruned values (2,183 of
  2,183 decoded cases).
- **Downstream:** wikidata-pq's four normalise calls (claims, labels, descriptions,
  aliases, with their lookup and pruned tables) on c0 and s90 gave the same files byte
  for byte as 1.5.0.
- **Tests:** the Python suite passed on the CI wheel (622 passed), and so did the bench's
  parity check.

## Measured

wikidata-pq's bench, quick tier, 3 rounds, bench 7.4.0, #314's CI wheel (423e900, run
40186690454) against the released 1.5.0 (lmmx/wikidata-pq#105, stacked on the 1.5.0 pin):

| | 1.5.0 | #314 | change |
|---|---|---|---|
| `process.normalise_in_schema` | 7.2 s (41%) | 7.3 s (41%) | **+1.3% ±1.3%** |
| CPU, process | 13.6 s ± 2.7 s | = | |
| ETA: pipeline | 6h50m18s | 6h46m53s | within noise |
| wall | 6.72 s ± 1.15 s | 6.66 s ± 1.08 s | within noise |
| peak RSS, one worker (GB) | 1.725 ± 0.048 | 2.018 ± 0.035 | **+0.293 (+17.0% ±3.1%)** |
| peak RSS, all processes (GB) | 3.902 ± 0.109 | 4.329 ± 0.134 | within noise |
| parity | | | passed |

Locally the claims call's peak RSS was higher in 5 of 5 interleaved runs on each chunk:
about 150 MB on c0 (1,510–1,558 MB against 1,351–1,387 MB) and 100 MB on s90.

## Why it did not pay

- **Spare capacity:** a builder grows by doubling, and its finished buffer keeps the
  capacity. Before, the 32-bit buffers' spare capacity was freed once `conform` had
  written exact-size 64-bit ones. With 64-bit builders, the spare capacity is twice the
  bytes, and it stays in the arrays handed to wikidata-pq for as long as it holds them.
- **No time saved:** each doubling copies twice the bytes when the offsets are 64-bit.
  That likely costs about what the cast it replaced did (not profiled apart: the bench
  shows only that the call's CPU did not fall).
- **Not tried:**
  - **Copying the 64-bit offsets into exact-size buffers once a batch is decoded:** that
    would bring the memory back, but it is a pass over the offsets just like the cast,
    after a costlier growth.
  - **Reserving each column's capacity in advance:** no estimate of a column's strings or
    items per batch is at hand before the batch is decoded.

## The label, description and alias calls

Profiled with 1.5.0's stage timings (`profile=True`), 5 runs each on c0 and s90:
- **The calls:** each is a read, a decode against its `avro_schema`, `conform` and a
  Parquet write.
- **The write dominates:** labels on c0 took about 0.07 s to write of 0.10 s wall
  (decode 0.02 s, read 0.01 s), descriptions 0.06–0.08 s of 0.08–0.11 s, and aliases
  0.04–0.08 s of 0.07–0.10 s.
- **Conform** was 0.001–0.002 s in the fastest runs.

Most of their cost is writing the nested table, which is the process step's output and
the partition step's input. There is no pass in genson to remove, so nothing was pursued.
